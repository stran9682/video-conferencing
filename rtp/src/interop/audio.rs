use std::{
    io,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use csv::Writer;
use bytes::{BufMut, Bytes, BytesMut};
use tokio::{net::UdpSocket, sync::mpsc};

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

pub async fn rtp_audio_sender(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    mut rx: mpsc::Receiver<EncodedAudio>,
) {    
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


    let mut buffer =  BytesMut::with_capacity(1500);

    loop {
        let sample = match rx.recv().await {
            Some(s) => s,
            None => continue,
        };

        //println!("Received an audio sample");

        let peers = peer_manager.get_peers();

        //println!("Number of peers: {}", peers.len());

        if peers.is_empty() {
            //println!("-> No peers");
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

        for addr in peers.iter() {
            let now = SystemTime::now();
            let time_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();

            match socket.send_to(&packet, addr).await {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to send to {}: {}", addr, e),
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
    socket: Arc<UdpSocket>,
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

    let mut buffer = [0u8; 1500];

    loop {
        let (bytes_read, _) = socket.recv_from(&mut buffer).await?;

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
        
        if let Err(e) = data_sender.try_send((header.sequence_number, duration_since.as_nanos())) {
            eprintln!("Audio receiver channel full, {}", e)
        }
    }
}
