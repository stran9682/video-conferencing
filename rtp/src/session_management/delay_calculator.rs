use std::{sync::Arc, time::Duration};

use bytes::BytesMut;

use crate::{
    packets::rtp::RTPHeader,
    session_management::peer_manager::{Fragment, PeerManager, PlayoutBufferNode},
};

pub struct DelayCalculator {
    active_delay: u32,
    delay_estimate: u32,
    first_time: bool,
    skew_threshold: i32,
}

impl DelayCalculator {
    pub fn new(skew_threshold: i32) -> Self {
        Self {
            active_delay: 0,
            delay_estimate: 0,
            first_time: true,
            skew_threshold,
        }
    }

    pub fn calculate_playout_time(
        &mut self,
        peer_manager: &Arc<PeerManager>,
        arrival_time: Duration,
        media_clock_rate: u32,
        data: BytesMut,
        rtp_header: &RTPHeader,
    ) -> Option<u32> {
        /*
            Calculating Base Playout time:

            M = T * R + offset
            d(n) = Arrival Time of Packet - Header Timestamp
            offset = Min(d(n-w)...d(n))
            base playout time = Timestamp + offset
        */

        // M = T * R + offset
        // don't worry that we're cutting off the bits
        // the method described in Perkin's book uses modulo arithmetic
        let arrival_time = arrival_time.as_millis() as u32 * (media_clock_rate / 1000);

        // d(n) = Arrival Time of Packet - Header Timestamp
        let difference = arrival_time.wrapping_sub(rtp_header.timestamp);

        // offset = Min(d(n-w)...d(n))
        // in the case when arrival time is smaller than timestamp.
        // wraparound comparison is handled here.
        let offset = peer_manager.peer_get_min_window(rtp_header.ssrc, difference)?;

        // base playout time = Timestamp + offset
        let base_playout_time = rtp_header.timestamp.wrapping_add(offset);

        let node = PlayoutBufferNode {
            rtp_timestamp: rtp_header.timestamp,
            playout_time: base_playout_time,
            coded_data: Vec::new(),
        };

        let fragment = Fragment::new(rtp_header.sequence_number, data.freeze());

        peer_manager.add_playout_node_to_peer(rtp_header.ssrc, node, fragment);

        // TODO: Something with this!!
        let adjustment = self.adjust_skew(difference);

        if rtp_header.marker {
            Some(base_playout_time)
        } else {
            None
        }
    }

    fn adjust_skew(&mut self, difference: u32) -> i32 {
        if self.first_time {
            self.first_time = false;
            self.delay_estimate = difference;
            self.active_delay = difference;
            return 0;
        }

        self.delay_estimate = (31 * self.delay_estimate + difference) / 32;

        let divergence = self.active_delay.wrapping_sub(self.delay_estimate) as i32;

        if divergence > self.skew_threshold {
            self.active_delay = self.delay_estimate;
            return self.skew_threshold;
        } else if divergence < -self.skew_threshold {
            self.active_delay = self.delay_estimate;
            return -self.skew_threshold;
        }

        0
    }
}
