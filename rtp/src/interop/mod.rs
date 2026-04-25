pub mod audio;
pub mod download;
pub mod upload;
pub mod video;
use rand::Rng;

use bytes::Bytes;
use local_ip_address::local_ip;

use core::slice;
use std::{
    io::{self},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::{Arc, OnceLock},
};

use tokio::{net::UdpSocket, runtime::Runtime, sync::mpsc};

use crate::{
    interop::{
        audio::{EncodedAudio, rtp_audio_sender},
        video::{EncodedFrame, ReleaseCallback, rtp_frame_sender},
    },
    packets::{RTPSession, rtcp::start_rtcp},
    quic::make_server_endpoint,
    session_management::{peer_manager::PeerManager, signaling_server::run_signaling_server},
};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

static FRAME_TX: OnceLock<mpsc::Sender<EncodedFrame>> = OnceLock::new();
static AUDIO_TX: OnceLock<mpsc::Sender<EncodedAudio>> = OnceLock::new();

const CHANNEL_BUFFER_SIZE: usize = 64;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum StreamType {
    Audio,
    Video,
}

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("Runtime creation failed. Loser"))
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_send_audio_sample(data: *const u8, len: usize, timestamp: u32) -> bool {
    let tx = match AUDIO_TX.get() {
        Some(tx) => tx,
        None => {
            eprintln!("Audio stream not initialized");
            return false;
        }
    };

    // Okay with copying here, we'd copy anyways swift side creating a pointer.
    let slice = unsafe { slice::from_raw_parts(data, len) };
    let data = Bytes::copy_from_slice(slice);

    let sample = EncodedAudio { data, timestamp };

    match tx.try_send(sample) {
        Ok(_) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            eprintln!("Warning: frame dropped - channel full");
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            eprintln!("Error: channel closed");
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_send_frame(
    data: *const u8,
    len: usize,
    context: *mut std::ffi::c_void,
    release_callback: ReleaseCallback,
    timestamp: u32,
) -> bool {
    let tx = match FRAME_TX.get() {
        Some(tx) => tx,
        None => {
            eprintln!("Video stream not initialized");
            return false;
        }
    };

    // zero copy
    let frame = EncodedFrame {
        data,
        len,
        context,
        release_callback,
        timestamp,
    };

    match tx.try_send(frame) {
        Ok(_) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            eprintln!("Warning: frame dropped - channel full");
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            eprintln!("Error: channel closed");
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run_runtime_server(stream: StreamType) {
    runtime().spawn(async move {
        if let Err(e) = network_loop_server(stream).await {
            eprintln!(
                "Something terrible happened. Not you though. You are amazing. Always: {}",
                e
            );
        }
    });
}

async fn network_loop_server(stream_type: StreamType) -> io::Result<()> {
    /*
       TODO: Handle a reconnection
       Some cases:
       -   Small network timeout (ip address and SSRC are the same)
           Honestly, not much needs to be done. Just wait out the network timeout
           SHOULD fix itself eventually

       -   Switch networks (IP address changes)
           Handle a full restart, meanwhile hopefully clients can remove the old peer
           Completely stop the backend and restart.
    */

    let local_ip = local_ip().unwrap();
    println!("New session initialized: {:?}", stream_type);

    let addr = Ipv4Addr::from_str(&local_ip.to_string()).unwrap();
    let rtp_addr = SocketAddr::new(IpAddr::V4(addr), 0);

    println!("attempting to make endpoint");
    let ssrc = {
        let mut rng = rand::rng();
        rng.next_u32() // there is a non-zero chance that SSRCs can colide...
    };

    let (endpoint, server_cert) = make_server_endpoint(rtp_addr)?;
    println!("Our {:?} address: {:?}", stream_type, endpoint.local_addr());

    let rtcp_port = endpoint.local_addr().unwrap().port() + 1;

    // Session management structs
    // we'll be using these throughout the program.
    let rtp_session = RTPSession::new(ssrc);
    let peer_manager = Arc::new(PeerManager::new(
        rtp_session,
        stream_type,
        endpoint,
        server_cert,
    ));

    println!("Binding RTCP socket");
    // RTCP: Sending to another peer's address is just their RTP address +1
    let rtcp_socket = UdpSocket::bind(format!("{}:{}", local_ip, rtcp_port)).await?;

    println!("{:?}, {}", stream_type, peer_manager.rtp_session.ssrc);

    // Signaling server thread
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        if let Err(e) = run_signaling_server(peer_manager_clone, stream_type).await {
            eprintln!("Signaling server error: {}", e);
        }
    });

    // TODO: Fix RTCP
    // RTCP Sender and receiver threads
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move { start_rtcp(rtcp_socket, peer_manager_clone, stream_type).await });

    // Video and Audio sender and receiver threads
    let sender_peers = Arc::clone(&peer_manager);
    match stream_type {
        StreamType::Video => {
            let (tx, rx) = mpsc::channel::<EncodedFrame>(CHANNEL_BUFFER_SIZE);

            FRAME_TX.set(tx).map_err(|_| {
                eprintln!("{:?} stream already initialized", stream_type);
                return io::Error::new(io::ErrorKind::AlreadyExists, "video stream already in use");
            })?;

            runtime().spawn(async move {
                rtp_frame_sender(sender_peers, rx).await;
            });

            //rtp_frame_receiver(socket, peer_manager, 90_000).await?
        }

        StreamType::Audio => {
            let (tx, rx) = mpsc::channel::<EncodedAudio>(CHANNEL_BUFFER_SIZE);

            AUDIO_TX.set(tx).map_err(|_| {
                eprintln!("{:?} stream already initialized", stream_type);
                return io::Error::new(io::ErrorKind::AlreadyExists, "audio stream already in use");
            })?;

            rtp_audio_sender(sender_peers, rx).await;

            // TODO:
            //rtp_audio_receiver(socket, peer_manager, 48_000).await
        }
    }

    Ok(())
}
