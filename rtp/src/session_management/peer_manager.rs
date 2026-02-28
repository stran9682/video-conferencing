use bytes::Bytes;
use dashmap::DashMap;
use std::time::Instant;
use std::{collections::VecDeque, net::SocketAddr};

use crate::packets::RTPSession;
use crate::packets::rtcp::reception_report::ReceptionReport;

static WINDOW_SIZE: usize = 50;
static MAX_DROPOUT: u16 = 3000;

pub struct PlayoutBufferNode {
    pub rtp_timestamp: u32,
    pub playout_time: u32,
    pub coded_data: Vec<Fragment>,
}

pub struct Fragment {
    pub extended_sequence_num: u32,
    pub sequence_num: u16,
    pub data: Bytes,
}

impl Fragment {
    pub fn new(sequence_num: u16, data: Bytes) -> Self {
        Self {
            sequence_num,
            data,
            extended_sequence_num: 0,
        }
    }
}

pub struct Peer {
    jitter: u32,
    max_sequence_number: Option<u16>,
    initial_sequence_number: Option<u16>,
    packets_received: u32,
    wrap_around_count: u32,
    swift_peer_model: *mut std::ffi::c_void,
    window: VecDeque<u32>,
    min_window: u32,
    playout_buffer: Vec<PlayoutBufferNode>,
    last_sr_timestamp: u32,
    delay_since_last_sr: Option<Instant>,
    expected_prior: u32,
    received_prior: u32,
}

impl Peer {
    pub fn new(swift_peer_model: *mut std::ffi::c_void) -> Self {
        Self {
            jitter: 0,
            delay_since_last_sr: None,
            last_sr_timestamp: 0,
            packets_received: 0,
            wrap_around_count: 0,
            max_sequence_number: None,
            initial_sequence_number: None,
            window: VecDeque::new(),
            min_window: u32::MAX,
            playout_buffer: Vec::with_capacity(100),
            swift_peer_model,
            expected_prior: 0,
            received_prior: 0,
        }
    }

    /// Determines the min arrival time along in a window,
    /// along with incrementing the packet count and recalculating the jitter
    pub fn set_and_get_min_window(&mut self, difference: u32) -> u32 {
        self.packets_received += 1;

        self.window.push_front(difference);
        let d = difference.wrapping_sub(self.window[0]) as i32;
        self.jitter = self.jitter + (d.abs() as u32 - self.jitter) / 16;

        if self.window.len() > WINDOW_SIZE {
            self.window.pop_back();
        }

        let min = self.window.iter().fold(self.window[0], |min, val| {
            if val.wrapping_sub(min) & 0x80000000 != 0 {
                *val
            } else {
                min
            }
        });

        self.min_window = min;

        return self.min_window;
    }

    pub fn add_node(&mut self, mut playout_buffer_node: PlayoutBufferNode, mut fragment: Fragment) {
        // accounting for wraparound
        if let Some(max_sequence_number) = self.max_sequence_number {
            let delta = fragment.sequence_num - max_sequence_number;

            if delta < MAX_DROPOUT {
                if fragment.sequence_num < max_sequence_number {
                    self.wrap_around_count += 1;
                }
                self.max_sequence_number = Some(fragment.sequence_num);
            } else if delta <= 65535 - 100 {
                // sequence number made a large jump
            } else {
                // misordered packet.
            }
        } else {
            // this is just to initalize it, usually the first frame
            // bad network conditions shouldn't need to be handled here
            self.max_sequence_number = Some(fragment.sequence_num);
            self.initial_sequence_number = Some(fragment.sequence_num);
        }

        // use extended timestamp for ordering
        fragment.extended_sequence_num =
            fragment.sequence_num as u32 + (65536 * self.wrap_around_count);

        let timestamp = playout_buffer_node.rtp_timestamp;

        match self
            .playout_buffer
            .binary_search_by_key(&timestamp, |node| node.rtp_timestamp)
        {
            Ok(index) => {
                let coded_data = &mut self.playout_buffer[index].coded_data;

                let index = coded_data
                    .binary_search_by_key(&fragment.extended_sequence_num, |frag| {
                        frag.extended_sequence_num
                    })
                    .unwrap_or_else(|i| i);

                coded_data.insert(index, fragment);
            }
            Err(index) => {
                playout_buffer_node.coded_data.push(fragment);
                self.playout_buffer.insert(index, playout_buffer_node);
            }
        }
    }

    pub fn update_last_sr_timestamp(&mut self, last_sr_timestamp: u32) {
        self.last_sr_timestamp = last_sr_timestamp;
        self.delay_since_last_sr = Some(Instant::now());
        self.expected_prior = self.expected_num_packets();
        self.received_prior = self.packets_received
    }

    pub fn max_extended_sequence_num(&self) -> u32 {
        let max_sequence = self.max_sequence_number.unwrap_or(0);
        max_sequence as u32 + (65536 * self.wrap_around_count)
    }

    pub fn expected_num_packets(&self) -> u32 {
        // I'm actually cheating a bit here,
        // according to Perkin's, you should use the last received sequence number, not highest one
        self.max_extended_sequence_num() - self.initial_sequence_number.unwrap_or(0) as u32
    }

    pub fn calculate_fraction_lost(&self) -> u8 {
        let expected_interval = self.expected_num_packets() - self.expected_prior;
        let received_inteval = self.packets_received - self.received_prior;
        let lost_inteval = expected_interval as i32 - received_inteval as i32;

        if expected_interval == 0 || lost_inteval <= 0 {
            return 0;
        }

        ((lost_inteval << 8) / expected_interval as i32) as u8
    }
}

// BAD BAD BAD!
unsafe impl Send for Peer {}
unsafe impl Sync for Peer {}


pub struct PeerManager {
    peers: DashMap<u32, Peer>,

    // to help take the load off of peers.
    // sending thread can just use this instead,
    // instead of blocking the receiving
    peer_addresses: DashMap<u32, SocketAddr>,
    pub rtp_session: RTPSession,
}

impl PeerManager {
    pub fn local_ssrc(&self) -> u32 {
        self.rtp_session.ssrc
    }

    pub fn local_rtp_addr(&self) -> SocketAddr {
        self.rtp_session.local_addr
    }

    pub fn new(rtp_session: RTPSession) -> Self {
        Self {
            peers: DashMap::new(),
            peer_addresses: DashMap::new(),
            rtp_session,
        }
    }

    pub fn get_context(&self, ssrc: u32) -> Option<*mut std::ffi::c_void> {
        if let Some(peer) = self.peers.get(&ssrc) {
            Some(peer.swift_peer_model)
        } else {
            None
        }
    }

    pub fn add_peer(
        &self,
        ssrc: u32,
        addr: SocketAddr,
        swift_peer_model: *mut std::ffi::c_void,
    ) -> bool {
        let peers = &self.peers;

        if !peers.contains_key(&ssrc) {
            peers.insert(ssrc, Peer::new(swift_peer_model));
            self.peer_addresses.insert(ssrc, addr);
            true
        } else {
            false
        }
    }

    pub fn peer_get_min_window(&self, ssrc: u32, difference: u32) -> Option<u32> {
        let peers = &self.peers;

        if let Some(mut found_peer) = peers.get_mut(&ssrc) {
            Some(found_peer.set_and_get_min_window(difference))
        } else {
            None
        }
    }

    pub fn add_playout_node_to_peer(
        &self,
        ssrc: u32,
        playout_buffer_node: PlayoutBufferNode,
        fragment: Fragment,
    ) {
        let peers = &self.peers;

        let Some(mut peer) = peers.get_mut(&ssrc) else {
            return;
        };

        peer.add_node(playout_buffer_node, fragment);
    }

    pub fn get_peers(&self) -> Vec<SocketAddr> {
        self.peer_addresses
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn pop_node(&self, ssrc: u32) -> Option<PlayoutBufferNode> {
        let mut peer = self.peers.get_mut(&ssrc)?;

        let Some(node) = peer.playout_buffer.pop() else {
            return None;
        };

        Some(node)
    }

    pub fn update_last_sr_timestamp(&self, ssrc: u32, last_sr_timestamp: u32) {
        if let Some(mut peer) = self.peers.get_mut(&ssrc) {
            peer.last_sr_timestamp = last_sr_timestamp;
        }
    }

    pub fn get_reception_reports(&self) -> Vec<ReceptionReport> {
        self.peers
            .iter()
            .map(|peer| {
                // this isn't even funny omg
                ReceptionReport {
                    reportee_ssrc: *peer.key(),
                    fraction_lost: peer.calculate_fraction_lost(),
                    total_lost: peer.expected_num_packets() - peer.packets_received,
                    extended_sequence_number: peer.max_extended_sequence_num(),
                    jitter: peer.jitter,
                    last_sr_timestamp: peer.last_sr_timestamp,
                    delay_since_last_sr: match peer.delay_since_last_sr {
                        None => 0,
                        Some(time) => {
                            let elapsed = time.elapsed();
                            let seconds = elapsed.as_secs();
                            (seconds * 65536) as u32
                        }
                    },
                }
            })
            .collect()
    }
}
