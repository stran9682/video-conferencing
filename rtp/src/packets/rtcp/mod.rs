use std::sync::Arc;

use bytes::{BufMut, BytesMut};
use rand::RngExt;
use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};

use crate::packets::rtcp::reception_report::ReceptionReport;
use crate::packets::rtcp::rtcp_header::RTCPHeader;
use crate::packets::rtcp::sender_report::SenderReport;
use crate::{interop::runtime, session_management::peer_manager::PeerManager};

mod reception_report;
mod rtcp_header;
mod sender_report;

pub async fn start_rtcp(
    socket: UdpSocket,
    peer_manager: Arc<PeerManager>,
) {
    let socket = Arc::new(socket);

    let socket_clone = Arc::clone(&socket);
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(socket_clone, peer_manager_clone).await;
    });

    rtcp_receiver(socket, peer_manager).await;
}

async fn rtcp_sender(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
) {
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
                length: 12
            };

            let sender_report = SenderReport {
                ssrc: peer_manager.local_ssrc(),
                ntp_time: 0,
                rtp_time: 0,
                packet_count: 0,
                octet_count: 0,
                reports: Vec::new()
            };

            let reception_report = ReceptionReport {
                ssrc: 0,
                fraction_lost: 0,
                total_lost: 0,
                last_sequence_number: 0,
                jitter: 0,
                last_sender_report: 0,
                delay: 0,
            };

            let mut packet = BytesMut::with_capacity(52);

            packet.put(header.serialize());
            packet.put(sender_report.serialize());
            packet.put(reception_report.serialize());


        }
    }
}

async fn rtcp_receiver(_socket: Arc<UdpSocket>, _peer_manager: Arc<PeerManager>) {
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
}
