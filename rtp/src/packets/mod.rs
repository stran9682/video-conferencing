use crate::{interop::StreamType, packets::rtp::rtp::RTPHeader};
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};

pub mod rtcp;
pub mod rtp;

#[derive(Debug)]
pub struct RTPSession {
    current_sequence_num: AtomicU16,
    packets_generated: AtomicU32,
    octets_sent: AtomicU32, // this is going to be same for every peer
    stream_type: StreamType,

    pub ssrc: u32,
}

impl RTPSession {
    pub fn new(ssrc: u32, stream_type: StreamType) -> Self {
        Self {
            octets_sent: AtomicU32::new(0),
            current_sequence_num: AtomicU16::new(0),
            packets_generated: AtomicU32::new(0),
            ssrc,
            stream_type,
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
            payload_type: self.stream_type as u8,
            sequence_number: self.current_sequence_num.load(Ordering::Relaxed),
            timestamp,
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
