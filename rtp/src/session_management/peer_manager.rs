use std::{collections::VecDeque, net::SocketAddr, u128::MAX};
use bytes::Bytes;
use dashmap::DashMap;

static WINDOW_SIZE: usize = 50;
static MAX_DROPOUT: u16 = 3000;

pub struct PlayoutBufferNode {
    pub arrival_time : u128,
    pub rtp_timestamp : u32,
    pub playout_time : u128, 
    pub coded_data : Vec<Fragment>
}

pub struct Fragment {
    pub extended_sequence_num : u32,
    pub sequence_num : u16,
    pub data : Bytes
}

impl Fragment {
    pub fn new (sequence_num : u16, data : Bytes) -> Self{
        Self {
            sequence_num,
            data, 
            extended_sequence_num: 0
        }
    }
}

pub struct Peer {
    max_sequence_number: Option<u16>,
    wrap_around_count: u32,
    swift_peer_model: *mut std::ffi::c_void,
    window : VecDeque<u128>,
    min_window : u128,
    playout_buffer : Vec<PlayoutBufferNode>
}

impl Peer {

    pub fn new (swift_peer_model: *mut std::ffi::c_void) -> Self {
        Self {
            wrap_around_count: 0,
            max_sequence_number: None,
            window: VecDeque::new(),
            min_window: MAX,
            playout_buffer: Vec::new(),
            swift_peer_model
        }
    }

    pub fn set_and_get_min_window (&mut self, difference : u128) -> u128{

        self.window.push_front(difference);

        if self.window.len() > WINDOW_SIZE {
            self.window.pop_back();
        }

        if let Some(&min_val) = self.window.iter().min() {
            self.min_window = min_val;
        }

        self.min_window = match self.window.iter().min() {
            Some(val) => *val,
            None => difference
        };

        return self.min_window
    }
}

// BAD BAD BAD!
unsafe impl Send for Peer { }
unsafe impl Sync for Peer { }

pub struct PeerManager {
    peers: DashMap<SocketAddr, Peer>,
    pub local_addr: SocketAddr,
}

impl PeerManager {
    pub fn new(local_addr: SocketAddr) -> Self {
        Self {
            peers: DashMap::new(),
            local_addr,
        }
    }

    pub fn get_context(&self, addr: SocketAddr) -> Option<*mut std::ffi::c_void>{
        if  self.peers.contains_key(&addr)  {
            let peer = self.peers.get(&addr);

            Some(peer.unwrap().swift_peer_model)
        } else {
            None
        }
    }

    pub fn add_peer(&self, addr: SocketAddr, swift_peer_model: *mut std::ffi::c_void) -> bool {
        let peers = &self.peers;
      
        if  !peers.contains_key(&addr) && addr != self.local_addr {
            peers.insert(addr, Peer::new(swift_peer_model));

            true
        } else {
            false
        }
    }

    pub fn peer_get_min_window(&self, addr: SocketAddr, difference: u128) -> u128 {
        let peers = &self.peers;

        if let Some(mut found_peer) = peers.get_mut(&addr) {
            found_peer.set_and_get_min_window(difference)
        } else {
            // peers.insert(addr, Peer{
            //     window: VecDeque::new(),
            //     min_window: difference,
            //     playout_buffer: Vec::new()
            // });

            difference
        }
    }

    pub fn add_playout_node_to_peer(&self, addr: SocketAddr, mut playout_buffer_node : PlayoutBufferNode, mut fragment: Fragment){
        let peers = &self.peers;

        let Some(mut peer) = peers.get_mut(&addr) else {
            return
        };


        // accounting for wraparound
        if let Some(max_sequence_number) = peer.max_sequence_number {
            let delta = fragment.sequence_num - max_sequence_number;

            if delta < MAX_DROPOUT {
                if fragment.sequence_num < max_sequence_number {
                    peer.wrap_around_count += 1;
                }
                peer.max_sequence_number = Some(fragment.sequence_num);

            } else if delta <= 65535 - 100 {
                // sequence number made a large jump

            } else {
                // misordered packet.
            }

        } else {
            // this is just to initalize it, usually the first frame
            // bad network conditions shouldn't need to be handled here
            peer.max_sequence_number = Some(fragment.sequence_num); 
        }

        // use extended timestamp for ordering
        fragment.extended_sequence_num = fragment.sequence_num as u32 + (65536 * peer.wrap_around_count);

        let timestamp = playout_buffer_node.rtp_timestamp;

        match peer.playout_buffer.binary_search_by_key(&timestamp, |node| node.rtp_timestamp) {
            Ok(index) => {

                let coded_data = &mut peer.playout_buffer[index].coded_data;

                let index = coded_data
                    .binary_search_by_key(&fragment.extended_sequence_num, |frag| frag.extended_sequence_num)
                    .unwrap_or_else(|i| i);

                coded_data.insert(index, fragment);
            }
            Err(index) => {
                playout_buffer_node.coded_data.push(fragment);
                peer.playout_buffer.insert(index, playout_buffer_node);
            }
        }
    }

    pub fn get_peers(&self) -> Vec<SocketAddr> {
        self.peers.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn pop_node(&self, addr: SocketAddr) -> Option<PlayoutBufferNode> {
        let mut peer = self.peers.get_mut(&addr)?;

        let Some(node) = peer.playout_buffer.pop() else {
            return  None;
        };

        Some(node)
    }
}