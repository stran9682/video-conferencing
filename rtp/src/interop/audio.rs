use std::{io, sync::Arc, time::{SystemTime, UNIX_EPOCH}};

use bytes::{BufMut, Bytes, BytesMut};
use tokio::{net::UdpSocket, sync::mpsc};

use crate::{packets::rtp::rtp::RTPHeader, session_management::{delay_calculator::calculate_playout_time, peer_manager::PeerManager}};

pub struct EncodedAudio {
    pub data: Bytes,
    pub timestamp: u32,
}

pub async fn rtp_audio_sender(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    mut rx: mpsc::Receiver<EncodedAudio>,
) {
    loop {
        let sample = match rx.recv().await {
            Some(s) => s,
            None => continue,
        };

        let peers = peer_manager.get_peers();

        if peers.is_empty() {
            continue;
        }

        let header = peer_manager.rtp_session.get_packet(
            false, 
            sample.timestamp, 
            sample.data.len() as u32
        );

        let mut packet = header.serialize();
        packet.put(sample.data);

        for addr in peers.iter() {
            match socket.send_to(&packet, addr).await {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to send to {}: {}", addr, e),
            }
        }
    }
}

pub async fn rtp_audio_receiver(
    socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    media_clock_rate: u32,
) -> io::Result<()> {
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

        // TODO: send back to swift
    }
}
