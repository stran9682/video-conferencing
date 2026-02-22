use crate::packets::rtp::rtp::RTPHeader;
use rand::Rng;
use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU16, Ordering},
};

pub mod rtcp;
pub mod rtp;

pub struct RTPSession {
    current_sequence_num: AtomicU16,
    pub ssrc: u32,
    pub local_addr: SocketAddr,
}

impl RTPSession {
    pub fn new(local_addr: SocketAddr) -> Self {
        let mut rng = rand::rng();

        Self {
            current_sequence_num: AtomicU16::new(0),
            ssrc: rng.next_u32(), // there is a non-zero chance that SSRCs can colide... 
            local_addr,
        }
    }

    pub fn get_packet(&self, is_last_unit: bool, timestamp: u32) -> RTPHeader {
        self.current_sequence_num.fetch_add(1, Ordering::Relaxed);

        RTPHeader {
            version: 2,
            padding: false,
            extension: false,
            marker: is_last_unit,
            payload_type: 0,
            sequence_number: self.current_sequence_num.load(Ordering::Relaxed),
            timestamp: timestamp,
            ssrc: self.ssrc,
            // csrc:
        }
    }
}
