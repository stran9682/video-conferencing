use std::{
    io,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use bytes::{BufMut, Bytes, BytesMut};
use csv::Writer;
use quinn::Connection;
use tokio::sync::mpsc;

use crate::{
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
    let mut wtr = Writer::from_path("audio_send_data.csv").unwrap();

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
                .rtp_session
                .get_packet(false, sample.timestamp, sample.data.len() as u32);

        //println!("Created a packet");
        header.serialize(&mut buffer);
        buffer.put(sample.data);

        //println!("packet size: {:?}", packet.len());
        let packet = buffer.split().freeze();

        for connection in peers.iter() {
            let now = SystemTime::now();
            let time_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();

            wtr.write_record(&[
                peer_manager.local_ssrc().to_string(), 
                header.sequence_number.to_string(), 
                time_since_epoch.as_nanos().to_string()
            ]).unwrap();

            match connection.send_datagram_wait(packet.clone()).await {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to send to {}: {}", connection.remote_address(), e),
            }
        }

        wtr.flush().unwrap();

        buffer.reserve(1500);
        //println!("Sent a packet")
    }
}

pub async fn rtp_audio_receiver(
    connection: Arc<Connection>,
    peer_manager: Arc<PeerManager>,
    media_clock_rate: u32,
) -> io::Result<()> {
    println!("Starting an audio receiver");

    loop {
        let mut data = match connection.read_datagram().await {
            Ok(data) => data,
            Err(e) => {
                let err = format!(
                    "Audio receiver of {} terminated {}",
                    connection.remote_address(),
                    e
                );

                eprintln!("{}", err);

                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, err));
            }
        };

        //println!("Got a packet!");

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

        let play_out_time = calculate_playout_time(
            &peer_manager,
            duration_since,
            media_clock_rate,
            data,
            &header,
        );

        let Some(sample) = peer_manager.pop_node(header.ssrc, header.timestamp) else {
            continue;
        };

        let Some(context) = peer_manager.get_context(header.ssrc) else {
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
