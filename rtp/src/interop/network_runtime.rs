use std::{
    ffi::c_void,
    io, slice,
    str::FromStr,
    sync::{Arc, OnceLock},
};

use bytes::Bytes;
use iroh::{
    Endpoint, PublicKey,
    endpoint::{Connection, presets},
    protocol::{AcceptError, ProtocolHandler, Router},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    interop::{
        StreamType,
        audio::{EncodedAudio, rtp_audio_receiver, rtp_audio_sender},
        runtime,
        video::{EncodedFrame, ReleaseCallback, rtp_frame_receiver, rtp_frame_sender},
    },
    packets::{rtcp::start_rtcp, rtp::rtp::RTPHeader},
    session_management::{
        peer_manager::{ConnectionData, PeerManager},
        signaling_server::{
            OpusArgs, swift_receive_audio_config, swift_receive_pps_sps, swift_remove_audio_peer,
            swift_remove_video_peer,
        },
    },
};

pub struct H264Parameters {
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
}

struct SwiftContext {
    context: *mut c_void,
}
unsafe impl Send for SwiftContext {}
unsafe impl Sync for SwiftContext {}

static PEER_VIDEO_CONTEXT: OnceLock<SwiftContext> = OnceLock::new();
static AUDIO_MANAGER_CONTEXT: OnceLock<SwiftContext> = OnceLock::new();

// TODO: eventually these should be updatable
static H264_PARAMETERS: OnceLock<H264Parameters> = OnceLock::new();
static OPUS_PARAMETERS: OnceLock<OpusArgs> = OnceLock::new();

static NODE: OnceLock<Router> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn set_video_callback(context: *mut c_void) {
    let _ = PEER_VIDEO_CONTEXT.set(SwiftContext { context });
}

#[unsafe(no_mangle)]
pub extern "C" fn set_audio_manger_context(context: *mut c_void) {
    let _ = AUDIO_MANAGER_CONTEXT.set(SwiftContext { context });
}

#[unsafe(no_mangle)]
pub extern "C" fn set_h264_args(
    pps: *const u8,
    pps_length: usize,
    sps: *const u8,
    sps_length: usize,
) {
    let pps = unsafe { slice::from_raw_parts(pps, pps_length) };
    let sps = unsafe { slice::from_raw_parts(sps, sps_length) };

    let _ = H264_PARAMETERS.set(H264Parameters {
        sps: pps.to_vec(),
        pps: sps.to_vec(),
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn set_opus_args(sample_rate: f64, channels: u32) {
    let _ = OPUS_PARAMETERS.set(OpusArgs {
        sample_rate,
        channels,
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn run_network_runtime(endpoint_str: *const u8, endpoint_str_length: usize) {
    let endpoint_str = if !endpoint_str.is_null() {
        let endpoint_slice = unsafe { slice::from_raw_parts(endpoint_str, endpoint_str_length) };

        match str::from_utf8(endpoint_slice) {
            Ok(str) => Some(str.to_owned()),
            Err(_) => None,
        }
    } else {
        None
    };

    runtime().spawn(async move {
        PEER_VIDEO_CONTEXT.wait();
        AUDIO_MANAGER_CONTEXT.wait();
        H264_PARAMETERS.wait();
        OPUS_PARAMETERS.wait();

        if let Err(e) = network_runtime(endpoint_str).await {
            eprintln!(
                "Something terrible happened. Not you though. You are amazing. Always: {}",
                e
            );
        }
    });
}

async fn network_runtime(endpoint_str: Option<String>) -> anyhow::Result<()> {
    let endpoint = Endpoint::bind(presets::N0).await?;
    endpoint.online().await;

    let peer_manager = Arc::new(PeerManager::new());
    let rtp_session = RTP::new(peer_manager.clone());

    // Video sending task
    let (tx, rx) = mpsc::channel::<EncodedFrame>(100);
    FRAME_TX.set(tx).map_err(|_| {
        io::Error::new(io::ErrorKind::AlreadyExists, "video stream already in use")
    })?;
    let video = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtp_frame_sender(video, rx).await;
    });

    // Audio sending task
    let (tx, rx) = mpsc::channel::<EncodedAudio>(100);
    AUDIO_TX.set(tx).map_err(|_| {
        io::Error::new(io::ErrorKind::AlreadyExists, "video stream already in use")
    })?;
    let audio = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtp_audio_sender(audio, rx).await;
    });

    let node = Router::builder(endpoint.clone())
        .accept("rtp", rtp_session)
        .spawn();

    // just so we don't drop it
    let _ = NODE.set(node);

    if let Some(endpoint_str) = endpoint_str {
        let response = connect(&endpoint, &endpoint_str, &peer_manager).await?;

        // Connect to everyone else now
        for addr in response.peers {
            let _ = connect(&endpoint, &addr, &peer_manager)
                .await
                .inspect_err(|e| eprintln!("Failed to connect to {}, Err: {} ", addr, e))?;
        }
    }

    anyhow::Ok(())
}

async fn connect(
    endpoint: &Endpoint,
    endpoint_str: &str,
    peer_manager: &Arc<PeerManager>,
) -> anyhow::Result<ConnectionArgs> {
    let public_key = PublicKey::from_str(endpoint_str)?;

    let connection = endpoint.connect(public_key, b"rtp").await?;
    let (mut send, mut recv) = connection.open_bi().await?;

    let request = construct_response(peer_manager);

    let request = serde_json::to_vec(&request).map_err(|e| AcceptError::from_boxed(e.into()))?;

    send.write_all(&request)
        .await
        .map_err(|e| AcceptError::from_boxed(e.into()))?;
    send.finish()?;

    let response = recv
        .read_to_end(1000)
        .await
        .map_err(|e| AcceptError::from_boxed(e.into()))?;

    let response: ConnectionArgs =
        serde_json::from_slice(&response).map_err(|e| AcceptError::from_boxed(e.into()))?;

    setup_swift_add_peer(peer_manager, &response, &connection);
    start_receivers(peer_manager, connection);

    anyhow::Ok(response)
}

#[derive(Debug)]
pub struct RTP {
    peer_manager: Arc<PeerManager>,
}

impl RTP {
    fn new(peer_manager: Arc<PeerManager>) -> Self {
        Self { peer_manager }
    }
}

impl ProtocolHandler for RTP {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let (mut send, mut recv) = connection.accept_bi().await?;

        // Recieve request
        let request = recv
            .read_to_end(1000)
            .await
            .map_err(|e| AcceptError::from_boxed(e.into()))?;

        let request: ConnectionArgs =
            serde_json::from_slice(&request).map_err(|e| AcceptError::from_boxed(e.into()))?;

        // Write a response back
        let response = construct_response(&self.peer_manager);
        let response =
            serde_json::to_vec(&response).map_err(|e| AcceptError::from_boxed(e.into()))?;
        send.write_all(&response)
            .await
            .map_err(|e| AcceptError::from_boxed(e.into()))?;
        send.finish()?;

        // Setup our swift side and add the peer to our active connections
        setup_swift_add_peer(&self.peer_manager, &request, &connection);

        start_receivers(&self.peer_manager, connection);

        Result::Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
struct ConnectionArgs {
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

static FRAME_TX: OnceLock<mpsc::Sender<EncodedFrame>> = OnceLock::new();
static AUDIO_TX: OnceLock<mpsc::Sender<EncodedAudio>> = OnceLock::new();

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

pub fn remove_peer(peer_manager: &Arc<PeerManager>, public_key: PublicKey) {
    let Some((peer_audio, peer_video)) = peer_manager.remove_peer(&public_key) else {
        return;
    };

    unsafe {
        swift_remove_audio_peer(
            AUDIO_MANAGER_CONTEXT.get().unwrap().context,
            peer_audio.0,
            peer_audio.1.swift_peer_model,
        );

        swift_remove_video_peer(
            peer_video.0,
            PEER_VIDEO_CONTEXT.get().unwrap().context,
            peer_video.1.swift_peer_model,
        );
    }
}

fn construct_response(peer_manager: &Arc<PeerManager>) -> ConnectionArgs {
    let h264_parameters = H264_PARAMETERS.get().unwrap();
    let opus_args = OPUS_PARAMETERS.get().unwrap();
    ConnectionArgs {
        video_ssrc: peer_manager.video_rtp_session.ssrc,
        audio_ssrc: peer_manager.audio_rtp_session.ssrc,
        peers: peer_manager
            .get_peers()
            .iter()
            .map(|con| con.remote_id().to_string())
            .collect(),
        pps: h264_parameters.pps.clone(),
        sps: h264_parameters.sps.clone(),
        sample_rate: opus_args.sample_rate,
        channels: opus_args.channels,
    }
}

fn setup_swift_add_peer(
    peer_manager: &Arc<PeerManager>,
    connection_args: &ConnectionArgs,
    connection: &Connection,
) {
    // Setting up audio inside swift
    let audio_manager_context = AUDIO_MANAGER_CONTEXT.wait();
    let swift_peer_audio = unsafe {
        swift_receive_audio_config(
            audio_manager_context.context,
            connection_args.sample_rate,
            connection_args.channels,
            connection_args.audio_ssrc,
        )
    };
    peer_manager.add_peer_data(
        connection_args.audio_ssrc,
        swift_peer_audio,
        0,
        StreamType::Audio,
    );

    // Setting up video inside swift
    let context = PEER_VIDEO_CONTEXT.wait();
    let swift_peer_video = unsafe {
        swift_receive_pps_sps(
            context.context,
            connection_args.pps.as_ptr(),
            connection_args.pps.len(),
            connection_args.sps.as_ptr(),
            connection_args.sps.len(),
            connection_args.video_ssrc,
        )
    };
    peer_manager.add_peer_data(
        connection_args.video_ssrc,
        swift_peer_video,
        3000,
        StreamType::Video,
    );

    let connection_data = ConnectionData::new(
        connection.clone(),
        connection_args.audio_ssrc,
        connection_args.video_ssrc,
    );
    peer_manager.add_connection(connection.remote_id(), connection_data);
}

fn start_receivers(peer_manager: &Arc<PeerManager>, connection: Connection) {
    // These will route packets to the correct task
    let (audio_tx, audio_rx) = mpsc::channel::<(RTPHeader, Bytes)>(100);
    let (frame_tx, frame_rx) = mpsc::channel::<(RTPHeader, Bytes)>(100);
    let (rtcp_tx, rtcp_rx) = mpsc::channel::<(Bytes, PublicKey)>(100);

    let audio = peer_manager.clone();
    runtime().spawn(async move {
        if let Err(e) = rtp_audio_receiver(audio_rx, audio, 48_000).await {
            eprintln!("audio receiver failed: {}", e);
        }
    });

    let video = peer_manager.clone();
    runtime().spawn(async move {
        if let Err(e) = rtp_frame_receiver(frame_rx, video, 90_000).await {
            eprintln!("frame receiver failed: {}", e);
        }
    });

    let rtcp = peer_manager.clone();
    runtime().spawn(async move { start_rtcp(rtcp, rtcp_rx).await });

    let peer_manager = peer_manager.clone();
    runtime().spawn(async move {
        loop {
            let mut packet = match connection.read_datagram().await {
                Ok(data) => data,
                Err(e) => {
                    let err = format!(
                        "Video receiver of {} terminated {}",
                        connection.remote_id(),
                        e
                    );
                    remove_peer(&peer_manager, connection.remote_id());

                    eprintln!("{}", err);
                    return;
                }
            };

            if packet[1] > 2 {
                let _ = rtcp_tx.send((packet, connection.remote_id())).await;
                eprintln!("RTCP receiver was full")
            } else {
                let header = RTPHeader::deserialize(&mut packet);

                if header.payload_type == 0 {
                    let _ = audio_tx.send((header, packet)).await;
                    eprintln!("Audio receive channel was full")
                } else {
                    let _ = frame_tx.send((header, packet)).await;
                    eprintln!("video receive channel was full")
                }
            }
        }
    });
}
