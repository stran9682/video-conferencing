use std::mem;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{io, sync::Arc};

use bytes::{BufMut, Bytes, BytesMut};
use csv::Writer;
use tokio::{net::UdpSocket, sync::mpsc};

use crate::interop::runtime;
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

pub async fn rtp_frame_sender(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    mut rx: mpsc::Receiver<EncodedFrame>,
) {
    

    let (data_sender,mut data_receiver) = mpsc::channel::<(u16, u128)>(256); 
    runtime().spawn(async move {
        let mut wtr = Writer::from_path("video_send_data.csv").unwrap();

        loop {
            let (sequence_num, timestamp) = data_receiver.recv().await.unwrap();

            wtr.write_record(&[
                sequence_num.to_string(),
                timestamp.to_string()
            ]).unwrap();

            wtr.flush().unwrap();
        }   
    });

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
                for addr in peers.iter() {

                    let now = SystemTime::now();
                    let time_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
 
                    match socket.send_to(&fragment.0, addr).await {
                        Ok(_) => {}
                        Err(e) => eprintln!("Failed to send to {}: {}", addr, e),
                    }

                    if let Err(e) = data_sender.try_send((fragment.1, time_since_epoch.as_nanos())) {
                        eprintln!("Video Writer channel full {}", e)
                    }
                }
            }
        }
    }
}

pub async fn rtp_frame_receiver(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    media_clock_rate: u32,
) -> io::Result<()> {
    let mut buffer = [0u8; 1500];

    // let _ = FRAME_OUTPUT.set(Arc::clone(&peer_manager));
    println!("Starting a video receiver");
    
    let (data_sender,mut data_receiver) = mpsc::channel::<(u16, u128)>(256); 
    runtime().spawn(async move {
        let mut wtr = Writer::from_path("video_receive_data.csv").unwrap();

        loop {
            let (sequence_num, timestamp) = data_receiver.recv().await.unwrap();

            wtr.write_record(&[
                sequence_num.to_string(),
                timestamp.to_string()
            ]).unwrap();

            wtr.flush().unwrap();
        }   
    });


    loop {
        let (bytes_read, _) = socket.recv_from(&mut buffer).await?;

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

        // Don't worry too much about copying, we do need to store it anyways
        let mut data = BytesMut::with_capacity(bytes_read);
        data.put_slice(&buffer[..bytes_read]);

        let header = RTPHeader::deserialize(&mut data);

        if let Err(e) = data_sender.try_send((header.sequence_number, duration_since.as_nanos())) {
            eprintln!("Video Receiver channel full: {}", e)
        }
    }
}
