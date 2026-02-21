use std::sync::Arc;

use tokio::net::UdpSocket;

use crate::{interop::runtime, session_management::peer_manager::PeerManager};

mod rtcp_header;
mod sender_report;
mod reception_report;

pub async fn start_rtcp(socket: UdpSocket, peer_manager: Arc<PeerManager>) {
    let socket = Arc::new(socket);

    let socket_clone = Arc::clone(&socket);
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(socket_clone, peer_manager_clone).await;
    });

    rtcp_receiver(socket, peer_manager).await;
}

async fn rtcp_sender(_socket: Arc<UdpSocket>, peer_manager: Arc<PeerManager>) {
    let _peers = peer_manager.get_peers();

    
}

async fn rtcp_receiver(_socket: Arc<UdpSocket>, _peer_manager: Arc<PeerManager>) {

}