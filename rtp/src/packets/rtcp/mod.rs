pub mod reception_report;
pub mod rtcp_header;
pub mod sender_report;

use std::sync::Arc;

use bytes::{BufMut, BytesMut};
use rand::RngExt;
use tokio::io;
use tokio::net::UdpSocket;
use tokio::time::{Duration, sleep};

use crate::packets::rtcp::reception_report::ReceptionReport;
use crate::packets::rtcp::rtcp_header::{PacketType, RTCPHeader};
use crate::packets::rtcp::sender_report::SenderReport;
use crate::{interop::runtime, session_management::peer_manager::PeerManager};

pub async fn start_rtcp(socket: UdpSocket, peer_manager: Arc<PeerManager>) {
    let socket = Arc::new(socket);

    let socket_clone = Arc::clone(&socket);
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(socket_clone, peer_manager_clone).await;
    });

    if let Err(e) = rtcp_receiver(socket, peer_manager).await {
        eprintln!("Something wrong with RTCP socket. Check: {}", e)
    };
}

async fn rtcp_sender(socket: Arc<UdpSocket>, peer_manager: Arc<PeerManager>) {
    loop {
        // RTCP bandwidith = 5% bit rate of a single stream of audio or video data
        // this is usually hard coded, so no need to track it.

        // The interval is how long to wait between sending RTCP packets
        // When more than 25% of the participants are senders:
        // Interval = average RTCP size * total number of members / RTCP bandwidth
        let interval = 5.0; // TODO: actually input right value

        // choose the minimum interval (usally 5 seconds) if calculated interval is less
        // if interval < 5.0 {
        //     interval = 5.0;
        // }

        // add some randomness
        // I = (Interval * random[0.5, 1.5])
        let random_interval = {
            let mut rng = rand::rng();
            rng.random_range(0.5..=1.5) * interval
        };

        // though if it's our first packet:
        // if (this is the first RTCP packet we are sending) {
        //     I *= 0.5
        // }

        // wait for packet time
        sleep(Duration::from_secs_f64(random_interval)).await;

        let peers = peer_manager.get_peers();

        for peer in peers {
            let header = RTCPHeader {
                padding: false,
                packet_type: rtcp_header::PacketType::SenderReport,
                count: 1,
                length: 12,
            };

            let sender_report = SenderReport {
                ssrc: peer_manager.local_ssrc(),
                ntp_time: 0,
                rtp_time: 0,
                packet_count: peer_manager.rtp_session.get_num_packets_generated(),
                octet_count: peer_manager.rtp_session.get_num_octets_sent(),
                reports: Vec::new(),
            };

            let reception_report = ReceptionReport {
                reportee_ssrc: 0,
                fraction_lost: 0,
                total_lost: 0,
                extended_sequence_number: 0,
                jitter: 0,
                last_sr_timestamp: 0,
                delay_since_last_sr: 0,
            };

            let mut packet = BytesMut::with_capacity(52);

            packet.put(header.serialize());
            packet.put(sender_report.serialize());
            packet.put(reception_report.serialize());
        }
    }
}

async fn rtcp_receiver(socket: Arc<UdpSocket>, peer_manager: Arc<PeerManager>) -> io::Result<()> {
    /*
       TODO:
       while packet
       Receive packet, read header
       match header type
           SR -> update statistics
           CNAME -> Associate names
           BYE -> Removal

       calculate next RTCP time to send

    */
    let mut buffer = [0u8; 1500];

    loop {
        let (bytes_read, _) = socket.recv_from(&mut buffer).await?;

        let mut packet = BytesMut::with_capacity(bytes_read);
        packet.put(&buffer[..bytes_read]);

        while packet.len() > 0 {
            let rtcp_header = RTCPHeader::deserialize(&mut packet);

            match rtcp_header.packet_type {
                PacketType::SenderReport => {
                    let sender_report = SenderReport::deserialize(&mut packet, rtcp_header.count);

                    let last_sr_timestamp = (sender_report.ntp_time >> 16 & 0xFFFFFFFF) as u32;

                    peer_manager.update_last_sr_timestamp(sender_report.ssrc, last_sr_timestamp);
                }
                _ => {}
            }
        }
    }
}
