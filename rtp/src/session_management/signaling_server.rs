use bytes::{BufMut, Bytes, BytesMut};
use core::slice;
use dashmap::DashSet;
use local_ip_address::local_ip;
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
        if let Err(e) = connect_to_signaling_server(host_addr_str, "video").await {
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

// inject an instance of a peer manager for the server to manage
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

    // splitting the request
    let data = Bytes::copy_from_slice(&buffer[..bytes_read]);

    let request: Vec<&[u8]> = data
        .split(|b| b == &0xA)
        .map(|line| line.strip_suffix(&[0xD]).unwrap_or(line))
        .collect();

    // get media type, should be thee first one item
    let Ok(media_type) = str::from_utf8(request[0]) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unable to parse media type",
        ));
    };

    let response = write_response(media_type).await?;

    socket.write_all(&response).await?;

    handle_request(&request, media_type).await?;

    Ok(())
}

pub async fn connect_to_signaling_server(
    server_addr: Option<String>,
    media_type: &str,
) -> io::Result<()> {
    // this is the case when you're the first person.
    // You don't have anyone to connect to
    let Some(server_addr) = server_addr else {
        return Ok(());
    };

    let packet = write_response(media_type).await?;

    //  this is silly, but connect to the first person and get their data and everyone's signalling address
    //  You'll only get their data! This is to make sure you connect to everyone
    //  addresses will be stored in vector
    let mut addresses: Vec<String> = Vec::new();
    add_peers(media_type, &server_addr, &packet, &mut addresses).await?;

    //  now, just loop through the addresses and get their data.
    //  The addresses are redundant since you got them already
    //  hence the empty vector
    for signaling_addr in &addresses {
        if let Err(e) = add_peers(
            media_type,
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
    media_type: &str,
    signaling_addr: &str,
    packet: &BytesMut,
    addresses: &mut Vec<String>,
) -> io::Result<()> {
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut socket = TcpStream::connect(signaling_addr).await?;

    socket.write_all(&packet).await?;

    let bytes_read = socket.read(&mut buffer).await?;

    if bytes_read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "No response from server",
        ));
    }

    let data = Bytes::copy_from_slice(&buffer[..bytes_read]);

    let data: Vec<&[u8]> = data
        .split(|b| b == &0xA)
        .map(|line| line.strip_suffix(&[0xD]).unwrap_or(line))
        .collect();

    handle_request(&data, media_type).await?;

    let signaling_addr: SocketAddr = signaling_addr
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    PEER_SPECIFICATIONS.get().unwrap().add_peer(signaling_addr);

    for response in &data[5..] {
        let Ok(str) = str::from_utf8(response) else {
            continue;
        };

        addresses.push(str.to_string());
    }

    Ok(())
}

async fn write_response(media_type: &str) -> io::Result<BytesMut> {
    let (stream_type, peer_manager) = match media_type {
        "video" => (StreamType::Video, FRAME_PEERS.get()),
        "audio" => (StreamType::Audio, AUDIO_PEERS.get()),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a valid type",
            ));
        }
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

    // writing the response
    let mut response = BytesMut::new();

    let header = format!(
        "{}\r\n{}\r\n{}\r\n",
        media_type, // 0
        signaling_addr.to_string(),
        peer_manager.local_addr(), // 1
    );
    response.put(header.as_bytes());
    response.put_u32(peer_manager.local_ssrc());
    response.put_slice(b"\r\n");

    let Some(specifications) = PEER_SPECIFICATIONS.get() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Specification manager not initialized",
        ));
    };

    let h264_args = specifications.self_h264_args.lock().await;

    match stream_type {
        StreamType::Video => {
            response.put_slice(&h264_args.pps); // 3
            response.put_slice(b"\r\n");
            response.put_slice(&h264_args.sps); // 4

            let signaling = specifications.get_peers();
            for addr in signaling {
                response.put_slice(b"\r\n");
                response.put(addr.to_string().as_bytes());
            }
        }
        StreamType::Audio => {
            // STILL WORKING ON IT!
        }
    }

    return Ok(response);
}

async fn handle_request(request: &Vec<&[u8]>, media_type: &str) -> io::Result<()> {
    let (specifications, peer_manager) = match media_type {
        "video" => (PEER_SPECIFICATIONS.get(), FRAME_PEERS.get()),
        "audio" => (PEER_SPECIFICATIONS.get(), AUDIO_PEERS.get()),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a valid type",
            ));
        }
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

    let signaling_addr: SocketAddr = str::from_utf8(request[1])
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Somemone sent you a faulty signaling address. {}", e),
            )
        })?
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let media_addr: SocketAddr = str::from_utf8(request[2])
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Somemone sent you a faulty media address. {}", e),
            )
        })?
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let Some(context) = PEER_VIDEO_CONTEXT.get() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Peer video manager likely not initialized",
        ));
    };

    // TOOO:    If the ip address & ssrc is the same, remove the old peer.
    //          then Update their specs

    let swift_peer_model = unsafe {
        swift_receive_pps_sps(
            context.context,
            request[4].as_ptr(),
            request[4].len(),
            request[5].as_ptr(),
            request[5].len(),
            media_addr.to_string().as_ptr(),
        )
    };

    let ssrc = u32::from_be_bytes(request[3][..4].try_into().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Somemone sent you a faulty u32 SSRC. {}", e),
        )
    })?);

    peer_manager.add_peer(ssrc, media_addr, swift_peer_model);
    specifications.add_peer(signaling_addr);

    Ok(())
}
