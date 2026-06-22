pub mod audio;
pub mod network_runtime;
pub mod video;

use bytes::Bytes;
use iroh::{PublicKey, endpoint::Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use tokio::{runtime::Runtime, sync::mpsc};

use crate::{
    interop::{audio::rtp_audio_receiver, network_runtime::remove_peer, video::rtp_frame_receiver},
    packets::{rtcp::start_rtcp, rtp::rtp::RTPHeader},
    session_management::peer_manager::PeerManager,
};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum StreamType {
    Audio = 0,
    Video = 1,
}

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("Runtime creation failed. Loser"))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub struct ConnectionArgs {
    video_ssrc: u32,
    audio_ssrc: u32,
    peers: Vec<String>,

    // Video
    pps: Vec<u8>,
    sps: Vec<u8>,

    // Audio
    sample_rate: f64,
    channels: u32,
}

fn start_receivers(peer_manager: &Arc<PeerManager>, connection: Connection) {
    println!("Creating new set of receivers");

    // These will route packets to the correct task
    let (audio_tx, audio_rx) = mpsc::channel::<(RTPHeader, Bytes)>(200);
    let (frame_tx, frame_rx) = mpsc::channel::<(RTPHeader, Bytes)>(200);
    let (rtcp_tx, rtcp_rx) = mpsc::channel::<(Bytes, PublicKey)>(200);

    let audio = peer_manager.clone();
    runtime().spawn(async move {
        let _ = rtp_audio_receiver(audio_rx, audio, 48_000)
            .await
            .inspect_err(|e| eprintln!("audio receiver failed: {}", e));
    });

    let video = peer_manager.clone();
    runtime().spawn(async move {
        let _ = rtp_frame_receiver(frame_rx, video, 90_000)
            .await
            .inspect_err(|e| eprintln!("frame receiver failed: {}", e));
    });

    println!("Audio video receivers created");

    let rtcp = peer_manager.clone();
    runtime().spawn(async move { start_rtcp(rtcp, rtcp_rx).await });

    let peer_manager = peer_manager.clone();
    runtime().spawn(async move {
        loop {
            let mut packet = match connection.read_datagram().await {
                Ok(data) => data,
                Err(e) => {
                    remove_peer(&peer_manager, connection.remote_id());
                    eprintln!(
                        "Video receiver of {} terminated {}",
                        connection.remote_id(),
                        e
                    );
                    return;
                }
            };

            // Perkins:
            // With the top bit stripped, the standard RTCP
            // packet types correspond to an RTP payload type in the range 72 to 76. This range is
            // reserved in the RTP specification and will not be used for valid RTP data packets, so
            // detection of packets in this range implies that the stream is misdirected.
            if packet[1] & 0x7F >= 72 {
                let _ = rtcp_tx
                    .send((packet, connection.remote_id()))
                    .await
                    .inspect_err(|e| eprintln!("RTCP receiver was full: {e}"));
            } else {
                let header = RTPHeader::deserialize(&mut packet);

                let tx = if header.payload_type == 0 {
                    &audio_tx
                } else {
                    &frame_tx
                };

                let _ = tx
                    .send((header, packet))
                    .await
                    .inspect_err(|e| eprintln!("RTP receiver was full: {e}"));
            }
        }
    });
}
