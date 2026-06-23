use std::mem::{self, zeroed};
use std::time::Instant;
use std::{io, sync::Arc};

use bytes::Bytes;
use tokio::sync::mpsc::{self, Receiver};

use crate::interop::StreamType;
use crate::packets::rtp::h264::{get_fragments, get_nal_units, rtp_to_avcc_h264};
use crate::packets::rtp::rtp::RTPHeader;
use crate::session_management::delay_calculator::calculate_playout_time;
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

pub struct H264Parameters {
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
}

pub struct EncodedFrame {
    pub data: *const u8,
    pub len: usize,
    pub context: *mut std::ffi::c_void,
    pub release_callback: ReleaseCallback,
    pub timestamp: u32,
}

impl Drop for EncodedFrame {
    fn drop(&mut self) {
        (self.release_callback)(self.context);
    }
}

// sometimes reasonable men do unreasonable things
unsafe impl Send for EncodedFrame {}

// TODO: Somewhere here handle a connection closing
pub async fn rtp_frame_sender(
    peer_manager: Arc<PeerManager>,
    mut rx: mpsc::Receiver<EncodedFrame>,
) {
    loop {
        let frame = match rx.recv().await {
            Some(f) => f,
            None => continue,
        };

        let peers = peer_manager.get_peers();

        if peers.is_empty() {
            continue;
        }

        // construct the slice on the SPOT!
        let data = unsafe { std::slice::from_raw_parts(frame.data, frame.len) };
        let timestamp = frame.timestamp;

        // we split the frame if it contains multiple NAL units, usually not though
        let nal_units = get_nal_units(data);
        let mut nal_units = nal_units.iter().peekable();

        while let Some(nal_unit) = nal_units.next() {
            // Split a NAL unit into multiple RTP packets
            let fragments = get_fragments(
                nal_unit,
                &peer_manager.video_rtp_session,
                nal_units.peek().is_none(), // last packet of the frame gets marked
                timestamp,
            );

            // send each packet to every peer
            for fragment in fragments {
                for connection in peers.iter() {
                    match connection.send_datagram_wait(fragment.clone()).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Failed to send to {}: {}", connection.remote_id(), e);
                        }
                    }
                }
            }
        }
    }
}

pub async fn rtp_frame_receiver(
    mut frame_rx: Receiver<(RTPHeader, Bytes)>,
    peer_manager: Arc<PeerManager>,
    media_clock_rate: u32,
) -> io::Result<()> {
    println!("Starting a video receiver");
    let instant = Instant::now();

    let mut max_timestamp: Option<u32> = None;

    loop {
        let (header, data) = match frame_rx.recv().await {
            Some(data) => data,
            None => {
                eprintln!("Video receiver channel closed:");

                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Video receiver channel closed",
                ));
            }
        };

        // This is VERY aggressive frame discarding
        // If we get a newer timestamp, discard the last one.
        // when we implement playout times, 
        // ideally we should just be going through the queue,
        // but for now this will do.
        if let Some(timestamp) = max_timestamp && timestamp != header.timestamp {
            let delta = header.timestamp - timestamp;

            if delta <= u32::MAX - 3000 {
                peer_manager.pop_node(header.ssrc, timestamp, StreamType::Video);
                max_timestamp = Some(header.timestamp);
            }
            else {
                // misordered packet
                peer_manager.pop_node(header.ssrc, header.timestamp, StreamType::Video);
                continue;
            }
        }
        else {
            max_timestamp = Some(header.timestamp);
        }

        let duration_since = instant.elapsed();

        let play_out_time = calculate_playout_time(
            &peer_manager,
            duration_since,
            media_clock_rate,
            data,
            &header,
            StreamType::Video,
        );

        // Send to swift
        if let Some(play_out_time) = play_out_time
            && header.marker
        {
            let Some(frame) =
                peer_manager.pop_node(header.ssrc, header.timestamp, StreamType::Video)
            else {
                continue;
            };

            let frame_bytes: Vec<Bytes> = frame
                .coded_data
                .into_iter()
                .map(|frame| frame.data)
                .collect();

            let mut frame_data = rtp_to_avcc_h264(frame_bytes);
            let frame_data_length = frame_data.len();

            let Some(context) = peer_manager.get_context(header.ssrc, StreamType::Video) else {
                eprintln!("Video context was not setup for peer: {}", header.ssrc);
                continue; // in case that the UI hasn't sent back the pointer to stream, just ignore
            };

            unsafe {
                swift_receive_frame(
                    context,
                    frame_data.as_mut_ptr() as *mut std::ffi::c_void,
                    frame_data_length,
                );
            }

            mem::forget(frame_data);
        }
    }
}
