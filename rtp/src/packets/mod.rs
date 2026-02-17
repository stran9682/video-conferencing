use std::{net::SocketAddr, sync::atomic::{AtomicU16, AtomicU32, Ordering}};
use rand::Rng;
use crate::packets::rtp::RTPHeader;

pub mod rtp;
pub mod rtcp;

pub struct RTPSession {
    current_sequence_num : AtomicU16,
    timestamp : AtomicU32,
    pub increment : u32,
    pub ssrc : u32,

    pub local_addr: SocketAddr,
    pub rtcp_addr: SocketAddr
}

impl RTPSession {
    pub fn new(increment: u32, local_addr: SocketAddr, rtcp_addr: SocketAddr) -> Self {
        let mut rng = rand::rng();

        Self { 
            current_sequence_num: AtomicU16::new(0), 
            timestamp: AtomicU32::new(0),
            increment, 
            ssrc: rng.next_u32(),
            local_addr,
            rtcp_addr
        }
    }

    pub fn next_packet (&self) {
       self.timestamp.fetch_add(self.increment, Ordering::Relaxed);
    }

    pub fn get_packet (&self, is_last_unit: bool) -> RTPHeader {
        self.current_sequence_num.fetch_add(1, Ordering::Relaxed);

        RTPHeader { 
            version: 2,
            padding: false,
            extension: false,
            marker: is_last_unit,
            payload_type: 0,
            sequence_number: self.current_sequence_num.load(Ordering::Relaxed),
            timestamp: self.timestamp.load(Ordering::Relaxed), 
            ssrc: self.ssrc, 
            // csrc:  
        }
    }
}