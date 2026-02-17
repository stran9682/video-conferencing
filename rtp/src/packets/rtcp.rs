use std::sync::Arc;

use bytes::{Buf, BufMut, BytesMut};
use tokio::net::UdpSocket;

use crate::{interop::runtime, session_management::peer_manager::PeerManager};

pub const RTP_VERSION: u8 = 2;
pub const VERSION_SHIFT: u8 = 6;
pub const VERSION_MASK: u8 = 0x3;
pub const PADDING_SHIFT: u8 = 5;
pub const PADDING_MASK: u8 = 0x1;
pub const COUNT_SHIFT: u8 = 0;
pub const COUNT_MASK: u8 = 0x1f;

pub const HEADER_LENGTH: usize = 4;
pub const COUNT_MAX: usize = (1 << 5) - 1;
pub const SSRC_LENGTH: usize = 4;
pub const SDES_MAX_OCTET_COUNT: usize = (1 << 8) - 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    Unsupported = 0,
    SenderReport = 200,              // RFC 3550, 6.4.1
    ReceiverReport = 201,            // RFC 3550, 6.4.2
    SourceDescription = 202,         // RFC 3550, 6.5
    Goodbye = 203,                   // RFC 3550, 6.6
    ApplicationDefined = 204,        // RFC 3550, 6.7 (unimplemented)
    TransportSpecificFeedback = 205, // RFC 4585, 6051
    PayloadSpecificFeedback = 206,   // RFC 4585, 6.3
    ExtendedReport = 207,            // RFC 3611
}

impl From<u8> for PacketType {
    fn from(b: u8) -> Self {
        match b {
            200 => PacketType::SenderReport,              // RFC 3550, 6.4.1
            201 => PacketType::ReceiverReport,            // RFC 3550, 6.4.2
            202 => PacketType::SourceDescription,         // RFC 3550, 6.5
            203 => PacketType::Goodbye,                   // RFC 3550, 6.6
            204 => PacketType::ApplicationDefined,        // RFC 3550, 6.7 (unimplemented)
            205 => PacketType::TransportSpecificFeedback, // RFC 4585, 6051
            206 => PacketType::PayloadSpecificFeedback,   // RFC 4585, 6.3
            207 => PacketType::ExtendedReport,            // RFC 3611
            _ => PacketType::Unsupported,
        }
    }
}

pub struct RTCPHeader {
    /// If the padding bit is set, this individual RTCP packet contains
    /// some additional padding octets at the end which are not part of
    /// the control information but are included in the length field.
    pub padding: bool,
    /// The number of reception reports, sources contained or FMT in this packet (depending on the Type)
    pub count: u8,
    /// The RTCP packet type for this packet
    pub packet_type: PacketType,
    /// The length of this RTCP packet in 32-bit words minus one,
    /// including the header and any padding.
    pub length: u16,
}

impl RTCPHeader {
    pub fn serialize(&self) -> BytesMut {

        // TODO: Adjust this number lol
        let mut buf = BytesMut::with_capacity(64);

        let b0 = (RTP_VERSION << VERSION_SHIFT)
            | ((self.padding as u8) << PADDING_SHIFT)
            | (self.count << COUNT_SHIFT);

        buf.put_u8(b0);
        buf.put_u8(self.packet_type as u8);
        buf.put_u16(self.length);


        buf
    }

    pub fn deserialize(packet: &mut BytesMut) -> RTCPHeader {
        let b0 = packet.get_u8();
        let version = (b0 >> VERSION_SHIFT) & VERSION_MASK;

        let padding = ((b0 >> PADDING_SHIFT) & PADDING_MASK) > 0;
        let count = (b0 >> COUNT_SHIFT) & COUNT_MASK;
        let packet_type = PacketType::from(packet.get_u8());
        let length = packet.get_u16();

        RTCPHeader {
            padding,
            count,
            packet_type,
            length,
        }
    }
}

pub async fn start_rtcp (
    socket: UdpSocket, 
    peer_manager: Arc<PeerManager>
) {
    let socket = Arc::new(socket);

    let socket_clone = Arc::clone(&socket);
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(socket_clone, peer_manager_clone).await;
    });

    rtcp_receiver(socket, peer_manager).await;
}

async fn rtcp_sender (
    socket: Arc<UdpSocket>, 
    peer_manager: Arc<PeerManager>
) {

}

async fn rtcp_receiver (
    socket: Arc<UdpSocket>, 
    peer_manager: Arc<PeerManager>
) {

}