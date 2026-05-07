use std::{io, sync::Arc, time::Instant};

use bytes::{BufMut, Bytes, BytesMut};
use tokio::sync::mpsc::{self, Receiver};

use crate::{
    interop::StreamType,
    packets::rtp::rtp::RTPHeader,
    session_management::{delay_calculator::calculate_playout_time, peer_manager::PeerManager},
};

unsafe extern "C" {
    fn swift_receive_sample(context: *mut std::ffi::c_void, audioData: *const u8, length: usize);
}

pub struct EncodedAudio {
    pub data: Bytes,
    pub timestamp: u32,
}

// TODO: Somewhere here handle a connection closing
pub async fn rtp_audio_sender(
    peer_manager: Arc<PeerManager>,
    mut rx: mpsc::Receiver<EncodedAudio>,
) {
    let mut buffer = BytesMut::with_capacity(1500);

    loop {
        let sample = match rx.recv().await {
            Some(s) => s,
            None => continue,
        };

        //println!("Received an audio sample");

        let peers = peer_manager.get_peers();

        //println!("Number of peers: {}", peers.len());

        if peers.is_empty() {
            continue;
        }

        let header =
            peer_manager
                .audio_rtp_session
                .get_packet(false, sample.timestamp, sample.data.len() as u32);

        //println!("Created a packet");
        header.serialize(&mut buffer);
        buffer.put(sample.data);

        //println!("packet size: {:?}", packet.len());
        let packet = buffer.split().freeze();

        for connection in peers.iter() {
            match connection.send_datagram_wait(packet.clone()).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to send to {}: {}", connection.remote_id(), e);
                }
            }
        }

        buffer.reserve(1500);

        //println!("Sent a packet")
    }
}

pub async fn rtp_audio_receiver(
    mut audio_rx: Receiver<(RTPHeader, Bytes)>,
    peer_manager: Arc<PeerManager>,
    media_clock_rate: u32,
) -> io::Result<()> {
    println!("Starting an audio receiver");

    let instant = Instant::now();

    loop {
        let (header, data) = match audio_rx.recv().await {
            Some(data) => data,
            None => {
                eprintln!("Video receiver channel closed:");

                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Video receiver channel closed",
                ));
            }
        };

        //println!("Got a packet!");

        let duration_since = instant.elapsed();

        let play_out_time = calculate_playout_time(
            &peer_manager,
            duration_since,
            media_clock_rate,
            data,
            &header,
            StreamType::Audio,
        );

        let Some(sample) = peer_manager.pop_node(header.ssrc, header.timestamp, StreamType::Audio)
        else {
            continue;
        };

        let Some(context) = peer_manager.get_context(header.ssrc, StreamType::Audio) else {
            continue; // in case that the UI hasn't sent back the pointer to stream, just ignore
        };

        let mut audio_data = BytesMut::new();

        for data in sample.coded_data {
            audio_data.put(data.data);
        }

        unsafe {
            swift_receive_sample(context, audio_data.as_ptr() as *const u8, audio_data.len());
        }
    }
}
