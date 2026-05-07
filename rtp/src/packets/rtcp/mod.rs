pub mod reception_report;
pub mod rtcp_header;
pub mod sender_report;

use std::sync::Arc;
use std::time::SystemTime;

use bytes::{BufMut, Bytes, BytesMut};
use iroh::PublicKey;
use rand::RngExt;
use tokio::io;
use tokio::sync::mpsc::Receiver;
use tokio::time::{Duration, sleep};

use crate::interop::StreamType;
use crate::packets::rtcp::rtcp_header::{PacketType, RTCPHeader};
use crate::packets::rtcp::sender_report::SenderReport;
use crate::{interop::runtime, session_management::peer_manager::PeerManager};

unsafe extern "C" {
    fn swift_send_cmclocktime() -> f64;
}

pub async fn start_rtcp(
    peer_manager: Arc<PeerManager>,
    rtcp_rx: Receiver<(Bytes, PublicKey)>,
) {
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(peer_manager_clone, StreamType::Video).await;
    });

    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(peer_manager_clone, StreamType::Audio).await;
    });

    if let Err(e) = rtcp_receiver(peer_manager, rtcp_rx).await {
        eprintln!("Something wrong with RTCP socket. Check: {}", e)
    };
}

async fn rtcp_sender(
    peer_manager: Arc<PeerManager>,
    stream_type: StreamType,
) {
    let mut first_packet = true;

    let (clock_rate, rtp_session) = match stream_type {
        StreamType::Audio => (48_000. , &peer_manager.audio_rtp_session),
        StreamType::Video => (90_000. , &peer_manager.video_rtp_session)
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

        // converting system time to ntp format:
        // graciously from: https://tickelton.gitlab.io/articles/ntp-timestamps/
        let now = SystemTime::now();
        let time_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();

        let seconds = time_since_epoch.as_secs() + 2_208_988_800;
        let fraction =
            ((time_since_epoch.subsec_micros() + 1) as f64 * (1u64 << 32) as f64 * 1.0e-6) as u32;
        let ntp = seconds << 32 | (fraction as u64);

        let sender_report = SenderReport {
            ssrc: peer_manager.audio_rtp_session.ssrc,
            ntp_time: ntp,
            rtp_time: unsafe { (swift_send_cmclocktime() * clock_rate) as u32 },
            packet_count: rtp_session.get_num_packets_generated(),
            octet_count: rtp_session.get_num_octets_sent(),
            reports: peer_manager.get_reception_reports(stream_type),
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

        let peers = peer_manager.get_peers();
        let packet = packet.freeze();

        for addr in peers {
            match addr.send_datagram_wait(packet.clone()).await {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to send RTCP to {}: {}", addr.remote_id(), e),
            }
        }
    }
}

async fn rtcp_receiver(
    peer_manager: Arc<PeerManager>, 
    mut rx: Receiver<(Bytes, PublicKey)>, 
) -> io::Result<()> {
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

    loop {
        let (mut data, public_key) = match rx.recv().await {
            Some(data) => data,
            None => {
                eprintln!("Video receiver channel closed:");

                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Video receiver channel closed",
                ));
            }
        };

        while data.len() > 0 {
            let header = RTCPHeader::deserialize(&mut data);

            match header.packet_type {
                PacketType::SenderReport => {
                    let sender_report = SenderReport::deserialize(&mut data, header.count);

                    // DETERMINE WHO THIS IS!
                    let stream_type = peer_manager
                        .determine_stream_type(&public_key, &sender_report.ssrc)
                        .ok_or(io::ErrorKind::ConnectionRefused)?;
                    
                    let last_sr_timestamp = (sender_report.ntp_time >> 16 & 0xFFFFFFFF) as u32;

                    peer_manager.update_last_sr_timestamp(sender_report.ssrc, last_sr_timestamp, stream_type);

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
