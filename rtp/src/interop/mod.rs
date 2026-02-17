pub mod audio;
pub mod video;

use local_ip_address::local_ip;

use std::{
    io::{self},
    sync::{Arc, OnceLock},
};

use tokio::{net::UdpSocket, runtime::Runtime, sync::mpsc};

use crate::{
    interop::{
        audio::{EncodedAudio, rtp_audio_receiver},
        video::{EncodedFrame, ReleaseCallback, rtp_frame_receiver, rtp_frame_sender},
    },
    packets::{RTPSession, rtcp::start_rtcp},
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
pub extern "C" fn rust_send_frame(
    data: *const u8,
    len: usize,
    context: *mut std::ffi::c_void,
    release_callback: ReleaseCallback,
) -> bool {
    let tx = match FRAME_TX.get() {
        Some(tx) => tx,
        None => {
            eprintln!("Video stream not initialized");
            return false;
        }
    };

    // zero copy?
    let frame = EncodedFrame {
        data,
        len,
        context,
        release_callback,
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

       -   Just disconnecting arruptly, no reconnection
           Clients need to detect a timeout period, and remove you
           RTCP delay since LSR might be useful!
    */

    let my_local_ip = local_ip().unwrap();
    let local_addr_str = my_local_ip.to_string() + ":0";
    println!("{local_addr_str}");

    let socket = UdpSocket::bind(local_addr_str.clone()).await?;
    let socket = Arc::new(socket);
    let rtcp_socket = UdpSocket::bind(local_addr_str).await?;

    // Session management objects
    // we'll be using these throughout the program.
    let rtp_session = RTPSession::new(
        match stream_type {
            StreamType::Audio => 0, // TODO: AUDIO !! WE'll GET THERE!
            StreamType::Video => 3000,
        },
        socket.local_addr()?,
        rtcp_socket.local_addr()?,
    );
    let peer_manager = Arc::new(PeerManager::new(rtp_session));

    // Signaling server thread
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        if let Err(e) = run_signaling_server(peer_manager_clone, stream_type).await {
            eprintln!("Signaling server error: {}", e);
        }
    });

    // RTCP Sender and receiver threads
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move { start_rtcp(rtcp_socket, peer_manager_clone) });

    // Video and Audio sender and receiver threads
    let sender_socket = Arc::clone(&socket);
    let sender_peers = Arc::clone(&peer_manager);
    match stream_type {
        StreamType::Video => {
            let (tx, rx) = mpsc::channel::<EncodedFrame>(CHANNEL_BUFFER_SIZE);

            FRAME_TX.set(tx).map_err(|_| {
                eprintln!("{:?} stream already initialized", stream_type);
                return io::Error::new(io::ErrorKind::AlreadyExists, "video stream already in use");
            })?;

            runtime().spawn(async move {
                rtp_frame_sender(sender_socket, sender_peers, rx).await;
            });

            rtp_frame_receiver(socket, peer_manager, 90_000).await
        }

        StreamType::Audio => {
            let (tx, _rx) = mpsc::channel::<EncodedAudio>(CHANNEL_BUFFER_SIZE);

            AUDIO_TX.set(tx).map_err(|_| {
                eprintln!("{:?} stream already initialized", stream_type);
                return io::Error::new(io::ErrorKind::AlreadyExists, "audio stream already in use");
            })?;

            // TODO : spawn an audio runtime.
            rtp_audio_receiver().await
        }
    }
}
