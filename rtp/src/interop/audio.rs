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
    interop::runtime, packets::rtp::rtp::RTPHeader, session_management::{delay_calculator::calculate_playout_time, peer_manager::PeerManager}
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
    
    let (data_sender,mut data_receiver) = mpsc::channel::<(u16, u128)>(256); 
    runtime().spawn(async move {
        let mut wtr = Writer::from_path("audio_send_data.csv").unwrap();

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

            match connection.send_datagram_wait(packet.clone()).await {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to send to {}: {}", connection.remote_address(), e),
            }

            if let Err(e) = data_sender.try_send((header.sequence_number, time_since_epoch.as_nanos())) {
                eprintln!("Audio writer channel full: {}", e)
            }
        }

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
    
    let (data_sender,mut data_receiver) = mpsc::channel::<(u16, u128)>(256); 
    runtime().spawn(async move {
       let mut wtr = Writer::from_path("audio_receive_data.csv").unwrap();

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

        if let Err(e) = data_sender.try_send((header.sequence_number, duration_since.as_nanos())) {
            eprintln!("Audio receiver channel full, {}", e)
        }
    }
}
