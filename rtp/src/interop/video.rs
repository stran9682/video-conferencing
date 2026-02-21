use std::time::{SystemTime, UNIX_EPOCH};
use std::{io, sync::Arc};

use bytes::{BufMut, Bytes, BytesMut};
use tokio::{net::UdpSocket, sync::mpsc};

use crate::packets::rtp::h264::{get_fragments, get_nal_units, rtp_to_avcc_h264};
use crate::packets::rtp::rtp::RTPHeader;
use crate::session_management::delay_calculator::DelayCalculator;
use crate::session_management::peer_manager::PeerManager;

//static FRAME_OUTPUT: OnceLock<Arc<PeerManager>> = OnceLock::new();

unsafe extern "C" {
    fn swift_receive_frame(
        context: *mut std::ffi::c_void,
        frameData: *mut std::ffi::c_void,
        frameDataLength: usize,
    );
}

pub type ReleaseCallback = extern "C" fn(*mut std::ffi::c_void);

pub struct EncodedFrame {
    pub data: *const u8,
    pub len: usize,
    pub context: *mut std::ffi::c_void,
    pub release_callback: ReleaseCallback,
}

impl Drop for EncodedFrame {
    fn drop(&mut self) {
        (self.release_callback)(self.context);
    }
}

// sometimes reasonable men do unreasonable things
unsafe impl Send for EncodedFrame {}

pub async fn rtp_frame_sender(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    mut rx: mpsc::Receiver<EncodedFrame>,
) {
    loop {
        let frame = match rx.recv().await {
            Some(f) => f,
            None => break,
        };

        let peers = peer_manager.get_peers();

        if peers.is_empty() {
            continue;
        }

        // construct the slice on the SPOT!
        let data = unsafe { std::slice::from_raw_parts(frame.data, frame.len) };

        let nal_units = get_nal_units(data);
        let mut nal_units = nal_units.iter().peekable();

        while let Some(nal_unit) = nal_units.next() {
            let fragments = get_fragments(
                nal_unit,
                &peer_manager.rtp_session,
                nal_units.peek().is_none(),
            );

            for fragment in fragments {
                for addr in peers.iter() {
                    match socket.send_to(&fragment, addr).await {
                        Ok(_) => {}
                        Err(e) => eprintln!("Failed to send to {}: {}", addr, e),
                    }
                }
            }
        }

        peer_manager.rtp_session.next_packet(); // this will increment the timestamp by 3000. (90kHz / 30 fps)
    }
}

pub async fn rtp_frame_receiver(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    media_clock_rate: u32,
) -> io::Result<()> {
    let mut buffer = [0u8; 1500];
    let mut delay_calculator = DelayCalculator::new(3000);

    // let _ = FRAME_OUTPUT.set(Arc::clone(&peer_manager));

    loop {
        let (bytes_read, addr) = socket.recv_from(&mut buffer).await?;

        // there's absolutely a bug where if the time switches playout will be messed up!
        // (ex: when there's daylight savings)
        // but the wall clock is "technically" more stable, and less susceptible to skew
        // bet big, take risks, that's the way.

        let now = SystemTime::now();

        let duration_since = now.duration_since(UNIX_EPOCH);

        let duration_since = match duration_since {
            Ok(yay) => yay,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "holy what happened??",
                ));
            }
        };

        let mut data = BytesMut::with_capacity(bytes_read);
        data.put_slice(&buffer[..bytes_read]);

        let header = RTPHeader::deserialize(&mut data);

        // TODO:
        // if RTP SSRC has been sent with a new address,
        // update it so the frame sender can be send it to updated address
        // though signalling may have handled it first, just redundancy

        let play_out_time = delay_calculator.calculate_playout_time(
            &peer_manager,
            duration_since,
            media_clock_rate,
            data,
            &header,
        );

        // Send to swift
        if let Some(play_out_time) = play_out_time {
            let Some(frame) = peer_manager.pop_node(header.ssrc) else {
                continue;
            };

            let frame_bytes: Vec<Bytes> = frame
                .coded_data
                .into_iter()
                .map(|frame| frame.data)
                .collect();

            let mut frame_data = rtp_to_avcc_h264(frame_bytes);
            let frame_data_length = frame_data.len();

            let Some(context) = peer_manager.get_context(header.ssrc) else {
                continue; // in case that the UI hasn't sent back the pointer to stream, just ignore
            };

            unsafe {
                swift_receive_frame(
                    context,
                    frame_data.as_mut_ptr() as *mut std::ffi::c_void,
                    frame_data_length,
                );
            }
        }

        //println!("{}: {}", addr.to_string(), bytes_read);
    }
}
