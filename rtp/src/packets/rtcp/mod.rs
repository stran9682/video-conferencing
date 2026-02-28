pub mod reception_report;
pub mod rtcp_header;
pub mod sender_report;

use std::sync::Arc;
use std::time::SystemTime;

use bytes::{BufMut, BytesMut};
use rand::RngExt;
use tokio::io;
use tokio::net::UdpSocket;
use tokio::time::{Duration, sleep};

use crate::interop::StreamType;
use crate::packets::rtcp::rtcp_header::{PacketType, RTCPHeader};
use crate::packets::rtcp::sender_report::SenderReport;
use crate::{interop::runtime, session_management::peer_manager::PeerManager};

unsafe extern "C" {
    fn swift_send_cmclocktime() -> f64;
}

pub async fn start_rtcp(
    socket: UdpSocket,
    peer_manager: Arc<PeerManager>,
    stream_type: StreamType,
) {
    let socket = Arc::new(socket);

    let socket_clone = Arc::clone(&socket);
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(socket_clone, peer_manager_clone, stream_type).await;
    });

    if let Err(e) = rtcp_receiver(socket, peer_manager).await {
        eprintln!("Something wrong with RTCP socket. Check: {}", e)
    };
}

async fn rtcp_sender(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    stream_type: StreamType,
) {
    let mut first_packet = true;

    let clock_rate: f64 = match stream_type {
        StreamType::Audio => 0.,
        StreamType::Video => 90000.,
    };

    loop {
        // RTCP bandwidith = 5% bit rate of a single stream of audio or video data
        // this is usually hard coded, so no need to track it.

        // TODO:
        // The interval is how long to wait between sending RTCP packets
        // When more than 25% of the participants are senders:
        // Interval = average RTCP size * total number of members / RTCP bandwidth
        let mut interval = 5.0; // i'm just defaulting to 5 for now.

        // choose the minimum interval if the calculated interval is less
        // if interval < 5.0 {
        //     interval = 5.0;
        // }

        // add some randomness
        interval = {
            let mut rng = rand::rng();
            rng.random_range(0.5..=1.5) * interval
        };

        // though, if it's our first packet, halve the sending time so it gets out faster
        if first_packet {
            interval *= 0.5;
            first_packet = false;
        }

        // wait for packet time
        sleep(Duration::from_secs_f64(interval)).await;

        let peers = peer_manager.get_peers();

        // converting system time to ntp format:
        // graciously from: https://tickelton.gitlab.io/articles/ntp-timestamps/
        let now = SystemTime::now();
        let time_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();

        let seconds = time_since_epoch.as_secs() + 2_208_988_800;
        let fraction =
            ((time_since_epoch.subsec_micros() + 1) as f64 * (1u64 << 32) as f64 * 1.0e-6) as u32;
        let ntp = seconds << 32 | (fraction as u64);

        let sender_report = SenderReport {
            ssrc: peer_manager.local_ssrc(),
            ntp_time: ntp,
            rtp_time: unsafe { (swift_send_cmclocktime() * clock_rate) as u32 },
            packet_count: peer_manager.rtp_session.get_num_packets_generated(),
            octet_count: peer_manager.rtp_session.get_num_octets_sent(),
            reports: peer_manager.get_reception_reports(),
        };

        let header = RTCPHeader {
            padding: false,
            packet_type: rtcp_header::PacketType::SenderReport,
            count: sender_report.reports.len() as u8,
            length: sender_report.length(),
        };

        // TOOD: Add the CNAME

        let mut packet = BytesMut::with_capacity(4 + sender_report.length() as usize);
        packet.put(header.serialize());
        packet.put(sender_report.serialize());

        for addr in peers {
            let rtcp_port = addr.port() + 1;
            let peer_ip = format!("{}:{}", addr.ip(), rtcp_port);

            match socket.send_to(&packet, peer_ip).await {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to send RTCP to {}: {}", addr, e),
            }
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

                    for report in sender_report.reports {
                        println!(
                            "{}: Jitter {}, {}",
                            report.reportee_ssrc, report.jitter, report.extended_sequence_number
                        )
                    }
                }
                _ => {}
            }
        }
    }
}
