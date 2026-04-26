use std::mem;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{io, sync::Arc};

use bytes::Bytes;
use csv::Writer;
use quinn::Connection;
use tokio::sync::mpsc;

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
    let mut wtr = Writer::from_path("video_send_data.csv").unwrap();

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
                &peer_manager.rtp_session,
                nal_units.peek().is_none(), // last packet of the frame gets marked
                timestamp,
            );

            // send each packet to every peer
            for fragment in fragments {
                for connection in peers.iter() {
                    let now = SystemTime::now();
                    let time_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();

                    wtr.write_record(&[
                        fragment.1.to_string(), 
                        time_since_epoch.as_nanos().to_string()
                    ]).unwrap();

                    match connection.send_datagram_wait(fragment.0.clone()).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Failed to send to {}: {}", connection.remote_address(), e)
                        }
                    }
                }
            }
        }

        wtr.flush().unwrap();
    }
}

pub async fn rtp_frame_receiver(
    connection: Arc<Connection>,
    peer_manager: Arc<PeerManager>,
    media_clock_rate: u32,
) -> io::Result<()> {
    // let _ = FRAME_OUTPUT.set(Arc::clone(&peer_manager));
    println!("Starting a video receiver");
    let mut wtr = Writer::from_path("video_receive_data.csv").unwrap();

    loop {
        let mut data = match connection.read_datagram().await {
            Ok(data) => data,
            Err(e) => {
                let err = format!(
                    "Video receiver of {} terminated {}",
                    connection.remote_address(),
                    e
                );

                eprintln!("{}", err);

                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, err));
            }
        };

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

        let header = RTPHeader::deserialize(&mut data);

        let clone = Arc::clone(&connection);
        peer_manager.add_connection(&header.ssrc, clone);

        wtr.write_record(&[
            header.sequence_number.to_string(), 
            duration_since.as_nanos().to_string()
        ]).unwrap();

        // disabling decoding just for testing
        // jitter buffer WIP
        // let play_out_time = calculate_playout_time(
        //     &peer_manager,
        //     duration_since,
        //     media_clock_rate,
        //     data,
        //     &header,
        // );
        let play_out_time: Option<u32> = None; 

        // Send to swift
        if let Some(play_out_time) = play_out_time
            && header.marker
        {
            let Some(frame) = peer_manager.pop_node(header.ssrc, header.timestamp) else {
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

            mem::forget(frame_data);
        }

        //println!("{}: {}", addr.to_string(), bytes_read);
    }
}
