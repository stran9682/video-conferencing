use std::sync::Arc;

use tokio::net::UdpSocket;

use crate::{interop::runtime, session_management::peer_manager::PeerManager};

mod reception_report;
mod rtcp_header;
mod sender_report;

pub async fn start_rtcp(
    socket: UdpSocket,
    peer_manager: Arc<PeerManager>,
    dedicated_bandwidth: u32,
) {
    let socket = Arc::new(socket);

    let socket_clone = Arc::clone(&socket);
    let peer_manager_clone = Arc::clone(&peer_manager);
    runtime().spawn(async move {
        rtcp_sender(socket_clone, peer_manager_clone, dedicated_bandwidth).await;
    });

    rtcp_receiver(socket, peer_manager).await;
}

async fn rtcp_sender(
    _socket: Arc<UdpSocket>,
    peer_manager: Arc<PeerManager>,
    dedicated_bandwidth: u32,
) {
    let _peers = peer_manager.get_peers();

    // RTCP bandwidith = 5% bit rate of a single stream of audio or
    // video data, multiplied by the typical number of simultaneous senders
    // just needs to be an estimate, no need to track it.

    // we are assuming that everyone will be a sender, so we'll be using this calculaation
    // Interval = average RTCP size * total number of members / RTCP bandwidth

    // choose minimum if interval is less (usally 5 seconds)
    // since our streams are point to point p2p, it SHOUlD be this always
    // If (Interval < minimum interval) {
    //    Interval = minimum interval
    // }

    // add some randomness
    // I = (Interval * random[0.5, 1.5])
    // next_rtcp_send_time = current_time + I

    // though if it's our first packet:
    // if (this is the first RTCP packet we are sending) {
    //     I *= 0.5
    // }

    // wait for packet time
    // then send
}

async fn rtcp_receiver(_socket: Arc<UdpSocket>, _peer_manager: Arc<PeerManager>) {
    /*
       TODO:
       while packet
       Receive packet, read header
       match header type
           SR -> update statistics
           CNAME -> Associate names
           BYE -> Removal

       calculate next RTCP time to send

    */
}
