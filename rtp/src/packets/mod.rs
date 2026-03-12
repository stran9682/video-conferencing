use crate::packets::rtp::rtp::RTPHeader;
use rand::Rng;
use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU16, AtomicU32, Ordering},
};

pub mod rtcp;
pub mod rtp;

pub struct RTPSession {
    current_sequence_num: AtomicU16,
    packets_generated: AtomicU32,
    octets_sent: AtomicU32, // this is going to be same for every peer

    pub ssrc: u32,
    pub local_addr: SocketAddr,
}

impl RTPSession {
    pub fn new(local_addr: SocketAddr) -> Self {
        let mut rng = rand::rng();

        Self {
            octets_sent: AtomicU32::new(0),
            current_sequence_num: AtomicU16::new(0),
            packets_generated: AtomicU32::new(0),
            ssrc: rng.next_u32(), // there is a non-zero chance that SSRCs can colide...
            local_addr,
        }
    }

    pub fn get_packet(&self, is_last_unit: bool, timestamp: u32, packet_length: u32) -> RTPHeader {
        self.current_sequence_num.fetch_add(1, Ordering::Relaxed);
        self.packets_generated.fetch_add(1, Ordering::Relaxed);
        self.octets_sent.fetch_add(packet_length, Ordering::Relaxed);

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

    pub fn get_num_packets_generated(&self) -> u32 {
        self.packets_generated.load(Ordering::Relaxed)
    }

    pub fn get_num_octets_sent(&self) -> u32 {
        self.octets_sent.load(Ordering::Relaxed)
    }
}
