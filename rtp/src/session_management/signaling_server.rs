use bytes::Bytes;
use core::slice;
use dashmap::DashSet;
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{Arc, OnceLock},
};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, OnceCell},
};

use crate::{
    interop::{StreamType, runtime},
    session_management::peer_manager::PeerManager,
};

const BUFFER_SIZE: usize = 1500;

static AUDIO_PEERS: OnceLock<Arc<PeerManager>> = OnceLock::new();
static FRAME_PEERS: OnceLock<Arc<PeerManager>> = OnceLock::new();
static LISTENER: OnceCell<TcpListener> = OnceCell::const_new();
static PEER_VIDEO_CONTEXT: OnceLock<PeerVideoManagerContext> = OnceLock::new();
static PEER_SPECIFICATIONS: OnceLock<PeerSpecifications> = OnceLock::new();
static SIGNALLING_ADDR: OnceLock<String> = OnceLock::new();

// TODO: update addr to use SSRC instead of address
unsafe extern "C" {
    fn swift_receive_pps_sps(
        context: *mut std::ffi::c_void,
        pps: *const u8,
        pps_length: usize,
        sps: *const u8,
        sps_length: usize,
        addr: *const u8,
    ) -> *mut std::ffi::c_void;
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum StreamTypeWithArgs {
    Video { pps: Vec<u8>, sps: Vec<u8> },
    Audio,
}

#[derive(Serialize, Deserialize)]
struct ServerArgs {
    signaling_address: String,
    local_rtp_address: String,
    ssrc: u32,
    stream_type: StreamTypeWithArgs,
    peer_signalling_addresses: Vec<String>,
}

struct PeerVideoManagerContext {
    context: *mut std::ffi::c_void,
}

// BAD BAD BAD!
unsafe impl Send for PeerVideoManagerContext {}
unsafe impl Sync for PeerVideoManagerContext {}

pub struct H264Args {
    sps: Bytes,
    pps: Bytes,
}

pub struct PeerSpecifications {
    peer_signaling_addresses: DashSet<SocketAddr>,
    self_h264_args: Mutex<H264Args>,
}

impl PeerSpecifications {
    pub fn new(h264_args: H264Args) -> Self {
        Self {
            peer_signaling_addresses: DashSet::new(),
            self_h264_args: Mutex::new(h264_args),
        }
    }

    pub fn get_peers(&self) -> HashSet<SocketAddr> {
        self.peer_signaling_addresses
            .iter()
            .map(|addr| addr.clone())
            .collect()
    }

    pub fn add_peer(&self, addr: SocketAddr) {
        self.peer_signaling_addresses.insert(addr);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_set_signalling_addr(host_addr: *const u8, host_addr_length: usize) {
    if !host_addr.is_null() {
        let host_addr_slice = unsafe { slice::from_raw_parts(host_addr, host_addr_length) };

        let Ok(host_addr_str) = str::from_utf8(host_addr_slice) else {
            return;
        };

        let _ = SIGNALLING_ADDR.set(host_addr_str.to_string());

        println!("Set address!, {}", SIGNALLING_ADDR.get().unwrap())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_send_video_callback(context: *mut std::ffi::c_void) {
    let _ = PEER_VIDEO_CONTEXT.set(PeerVideoManagerContext { context });
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_send_h264_config(
    pps: *const u8,
    pps_length: usize,
    sps: *const u8,
    sps_length: usize,
) {
    let pps = unsafe { slice::from_raw_parts(pps, pps_length) };

    let pps = Bytes::copy_from_slice(pps);

    let sps = unsafe { slice::from_raw_parts(sps, sps_length) };

    let sps = Bytes::copy_from_slice(sps);

    // updating your specs vs just setting them
    // when we update, we don't want to get rid of all your friends!
    if let Some(old_specs) = PEER_SPECIFICATIONS.get() {
        let mut old_specs = old_specs.self_h264_args.blocking_lock();
        *old_specs = H264Args { sps, pps }
    } else {
        let _ = PEER_SPECIFICATIONS.set(PeerSpecifications::new(H264Args { sps, pps }));
    }

    let host_addr_str = match SIGNALLING_ADDR.get() {
        Some(addr) => Some(addr.to_owned()),
        None => None,
    };

    runtime().spawn(async move {
        println!("Making a request!");

        if let Err(e) = connect_to_signaling_server(host_addr_str, StreamType::Video).await {
            eprintln!("Failed to connect to signaling server, {}", e)
        }

        // TOOD: If the connection fails when you update your specs, try another peer
    });
}

async fn listener() -> &'static TcpListener {
    LISTENER
        .get_or_init(|| async {
            let local_ip = local_ip().unwrap();
            TcpListener::bind(local_ip.to_string() + ":0")
                .await
                .unwrap()
        })
        .await
}

/// inject an instance of a peer manager for the server to manage
pub async fn run_signaling_server(
    peer_manager: Arc<PeerManager>,
    stream_type: StreamType,
) -> io::Result<()> {
    let res = match stream_type {
        StreamType::Audio => AUDIO_PEERS.set(Arc::clone(&peer_manager)),
        StreamType::Video => FRAME_PEERS.set(Arc::clone(&peer_manager)),
    };

    // return early. Do NOT run another instance of the server!
    if res.is_err() {
        return Ok(());
    }

    println!("{}", listener().await.local_addr().unwrap());

    loop {
        let (mut socket, client_addr) = match listener().await.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
                continue;
            }
        };

        // just twaddle until we get our own specs,
        // awaiting a connection should hold this off
        if PEER_SPECIFICATIONS.get().is_none() {
            continue;
        }

        println!("Request from {}", client_addr.to_string());

        runtime().spawn(async move {
            if let Err(e) = handle_signaling_client(&mut socket).await {
                eprintln!("Signaling error with {}: {}", client_addr, e);
            }
        });
    }
}

async fn handle_signaling_client(socket: &mut TcpStream) -> io::Result<()> {
    let mut buffer = [0; BUFFER_SIZE];

    let bytes_read = socket.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Ok(());
    }

    // parsing the request
    let request: ServerArgs = serde_json::from_slice(&buffer[..bytes_read]).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Could not parse request. {}", e),
        )
    })?;

    let response = write_response(&request.stream_type).await?;

    socket.write_all(&response.as_bytes()).await?;

    handle_request(&request).await?;

    Ok(())
}

pub async fn connect_to_signaling_server(
    server_addr: Option<String>,
    media_type: StreamType,
) -> io::Result<()> {
    // this is the case when you're the first person.
    // You don't have anyone to connect to
    let Some(server_addr) = server_addr else {
        return Ok(());
    };

    let Some(specifications) = PEER_SPECIFICATIONS.get() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Specification manager not initialized",
        ));
    };

    let h264_args = specifications.self_h264_args.lock().await;

    let response_args = match media_type {
        StreamType::Audio => StreamTypeWithArgs::Audio,
        StreamType::Video => StreamTypeWithArgs::Video {
            pps: h264_args.pps.to_vec(),
            sps: h264_args.sps.to_vec(),
        },
    };

    let packet = write_response(&response_args).await?;

    //  this is silly, but connect to the first person and get their data and everyone's signalling address
    //  You'll only get their data! This is to make sure you connect to everyone
    //  addresses will be stored in vector
    let mut addresses: Vec<String> = Vec::new();
    add_peers(&server_addr, &packet, &mut addresses).await?;

    //  now, just loop through the addresses and get their data.
    //  The addresses are redundant since you got them already
    //  hence the empty vector

    println!("{:?}", addresses);
    for signaling_addr in &addresses {
        if let Err(e) = add_peers(
            signaling_addr,
            &packet,
            &mut Vec::with_capacity(addresses.len()),
        )
        .await
        {
            eprint!("Error! : {}", e);
            continue;
        }
    }

    Ok(())
}

async fn add_peers(
    signaling_addr: &str,
    packet: &str,
    addresses: &mut Vec<String>,
) -> io::Result<()> {
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut socket = TcpStream::connect(signaling_addr).await?;

    socket.write_all(packet.as_bytes()).await?;

    let bytes_read = socket.read(&mut buffer).await?;

    if bytes_read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "No response from server",
        ));
    }

    let response: ServerArgs = serde_json::from_slice(&buffer[..bytes_read]).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Could not parse request. {}", e),
        )
    })?;

    handle_request(&response).await?;

    let signaling_addr: SocketAddr = signaling_addr
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    PEER_SPECIFICATIONS.get().unwrap().add_peer(signaling_addr);

    for response in response.peer_signalling_addresses {
        addresses.push(response);
    }

    Ok(())
}

async fn write_response(media_type: &StreamTypeWithArgs) -> io::Result<String> {
    let peer_manager = match media_type {
        StreamTypeWithArgs::Audio => AUDIO_PEERS.get(),
        StreamTypeWithArgs::Video { pps: _, sps: _ } => FRAME_PEERS.get(),
    };

    let Some(peer_manager) = peer_manager else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Peer manager not initialized",
        ));
    };

    let Ok(signaling_addr) = listener().await.local_addr() else {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Failed to get signaling address",
        ));
    };

    let Some(specifications) = PEER_SPECIFICATIONS.get() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Specification manager not initialized",
        ));
    };

    // writing the response
    let response = ServerArgs {
        signaling_address: signaling_addr.to_string(),
        local_rtp_address: peer_manager.local_rtp_addr().to_string(),
        ssrc: peer_manager.local_ssrc(),
        stream_type: media_type.clone(),
        peer_signalling_addresses: specifications
            .get_peers()
            .iter()
            .map(|addr| addr.to_string())
            .collect(),
    };

    let json_response = serde_json::to_string(&response)?;

    return Ok(json_response);
}

async fn handle_request(request: &ServerArgs) -> io::Result<()> {
    let (specifications, peer_manager) = match request.stream_type {
        StreamTypeWithArgs::Video { pps: _, sps: _ } => {
            (PEER_SPECIFICATIONS.get(), FRAME_PEERS.get())
        }
        StreamTypeWithArgs::Audio => (PEER_SPECIFICATIONS.get(), AUDIO_PEERS.get()),
    };

    let Some(peer_manager) = peer_manager else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Peer manager not initialized",
        ));
    };

    let Some(specifications) = specifications else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Specification manager not initialized",
        ));
    };

    // TOOO:    If the ip address & ssrc is the same, remove the old peer ,then update their specs

    let media_addr: SocketAddr = request
        .local_rtp_address
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    match &request.stream_type {
        StreamTypeWithArgs::Audio => {
            // TODO: We'll get there!
        }
        StreamTypeWithArgs::Video { pps, sps } => {
            let Some(context) = PEER_VIDEO_CONTEXT.get() else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Peer video manager likely not initialized",
                ));
            };

            let swift_peer_model = unsafe {
                swift_receive_pps_sps(
                    context.context,
                    pps.as_ptr(),
                    pps.len(),
                    sps.as_ptr(),
                    sps.len(),
                    media_addr.to_string().as_ptr(),
                )
            };

            peer_manager.add_peer(request.ssrc, media_addr, swift_peer_model);
        }
    }

    let signaling_addr: SocketAddr = request
        .signaling_address
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    specifications.add_peer(signaling_addr);

    Ok(())
}
