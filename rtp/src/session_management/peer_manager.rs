use bytes::Bytes;
use dashmap::DashMap;
use iroh::PublicKey;
use iroh::endpoint::Connection;
use rand::Rng;
use std::collections::VecDeque;
use std::time::Instant;

use crate::interop::StreamType;
use crate::packets::RTPSession;
use crate::packets::rtcp::reception_report::ReceptionReport;
use crate::session_management::delay_calculator::PeerDelay;

static WINDOW_SIZE: usize = 50;
static MAX_DROPOUT: u16 = 3000;

#[derive(Debug)]
pub struct PlayoutBufferNode {
    pub rtp_timestamp: u32,
    pub playout_time: u32,
    pub coded_data: Vec<Fragment>,
}

#[derive(Debug)]
pub struct Fragment {
    pub extended_sequence_num: u32,
    pub sequence_num: u16,
    pub data: Bytes,
}

impl Fragment {
    pub fn new(sequence_num: u16, data: Bytes) -> Self {
        Self {
            sequence_num,
            data,
            extended_sequence_num: 0,
        }
    }
}

#[derive(Debug)]
pub struct Peer {
    ///  variance in arrival time
    jitter: u32,

    /// highest sequence number currently received from this peer         
    max_sequence_number: Option<u16>,

    /// first sequence number received         
    initial_sequence_number: Option<u16>,

    /// number of packets received from this peer,
    /// can differ from max-initial when packets are lost
    packets_received: u32,

    /// number of times the sequence number has rolled over from max u16 value          
    wrap_around_count: u32,

    /// the swift context that will be receiving and decoding the payload
    pub swift_peer_model: *mut std::ffi::c_void,

    /// Stores the arrival time of the WINDOW_SIZE most recent packets
    window: VecDeque<u32>,

    /// packet in window with the earliest arrival time
    min_window: u32,

    /// buffer where frames with the same timestamp are grouped together
    playout_buffer: Vec<PlayoutBufferNode>,

    /// middle 32 bytes of the NTP timestamp as received of the last SR from this peer
    last_sr_timestamp: u32,

    /// Time since the last SR has been received
    delay_since_last_sr: Option<Instant>,

    /// the expected number of packets received when the last SR was sent
    expected_prior: u32,

    /// the received number of packets when the last SR was sent
    received_prior: u32,

    skew_calculator: PeerDelay,
}

impl Peer {
    pub fn new(swift_peer_model: *mut std::ffi::c_void, skew_threshold: i32) -> Self {
        Self {
            jitter: 0,
            delay_since_last_sr: None,
            last_sr_timestamp: 0,
            packets_received: 0,
            wrap_around_count: 0,
            max_sequence_number: None,
            initial_sequence_number: None,
            window: VecDeque::new(),
            min_window: u32::MAX,
            playout_buffer: Vec::with_capacity(100),
            swift_peer_model,
            expected_prior: 0,
            received_prior: 0,
            skew_calculator: PeerDelay::new(skew_threshold),
        }
    }

    fn is_timed_out(&self) -> bool {
        let Some(last_sr_time) = self.delay_since_last_sr else {
            return false;
        };

        return last_sr_time.elapsed().as_secs() > 10;
    }

    /// Determines the min arrival time along in a window,
    /// along with incrementing the packet count and recalculating the jitter
    fn set_and_get_min_window(&mut self, difference: u32) -> u32 {
        self.packets_received += 1;

        self.window.push_front(difference);
        let d = difference.wrapping_sub(self.window[0]) as i32;
        self.jitter = self.jitter + (d.abs() as u32 - self.jitter) / 16;

        if self.window.len() > WINDOW_SIZE {
            self.window.pop_back();
        }

        let min = self.window.iter().fold(self.window[0], |min, val| {
            if val.wrapping_sub(min) & 0x80000000 != 0 {
                *val
            } else {
                min
            }
        });

        self.min_window = min;

        return self.min_window;
    }

    pub fn add_node(&mut self, mut playout_buffer_node: PlayoutBufferNode, mut fragment: Fragment) {
        // accounting for wraparound
        if let Some(max_sequence_number) = self.max_sequence_number {
            let delta = fragment.sequence_num - max_sequence_number;

            if delta < MAX_DROPOUT {
                if fragment.sequence_num < max_sequence_number {
                    self.wrap_around_count += 1;
                }
                self.max_sequence_number = Some(fragment.sequence_num);
            } else if delta <= 65535 - 100 {
                // sequence number made a large jump
            } else {
                // misordered packet.
            }
        } else {
            // this is just to initalize it, usually the first frame
            // bad network conditions shouldn't need to be handled here
            self.max_sequence_number = Some(fragment.sequence_num);
            self.initial_sequence_number = Some(fragment.sequence_num);
        }

        // use extended timestamp for ordering
        fragment.extended_sequence_num =
            fragment.sequence_num as u32 + (65536 * self.wrap_around_count);

        let timestamp = playout_buffer_node.rtp_timestamp;

        match self
            .playout_buffer
            .binary_search_by_key(&timestamp, |node| node.rtp_timestamp)
        {
            Ok(index) => {
                let coded_data = &mut self.playout_buffer[index].coded_data;

                let index = coded_data
                    .binary_search_by_key(&fragment.extended_sequence_num, |frag| {
                        frag.extended_sequence_num
                    })
                    .unwrap_or_else(|i| i);

                coded_data.insert(index, fragment);
            }
            Err(index) => {
                playout_buffer_node.coded_data.push(fragment);
                self.playout_buffer.insert(index, playout_buffer_node);
            }
        }
    }

    fn update_last_sr_timestamp(&mut self, last_sr_timestamp: u32) {
        self.last_sr_timestamp = last_sr_timestamp;
        self.delay_since_last_sr = Some(Instant::now());
        self.expected_prior = self.expected_num_packets();
        self.received_prior = self.packets_received
    }

    fn max_extended_sequence_num(&self) -> u32 {
        let max_sequence = self.max_sequence_number.unwrap_or(0);
        max_sequence as u32 + (65536 * self.wrap_around_count)
    }

    fn expected_num_packets(&self) -> u32 {
        // I'm actually cheating a bit here,
        // according to Perkin's, you should use the last received sequence number, not highest one
        self.max_extended_sequence_num() - self.initial_sequence_number.unwrap_or(0) as u32
    }

    fn calculate_fraction_lost(&self) -> u8 {
        let expected_interval = self.expected_num_packets() - self.expected_prior;
        let received_inteval = self.packets_received - self.received_prior;
        let lost_inteval = expected_interval as i32 - received_inteval as i32;

        if expected_interval == 0 || lost_inteval <= 0 {
            return 0;
        }

        ((lost_inteval << 8) / expected_interval as i32) as u8
    }
}

// BAD BAD BAD!
unsafe impl Send for Peer {}
unsafe impl Sync for Peer {}

#[derive(Debug)]
pub struct ConnectionData {
    connection: Connection,
    audio_ssrc: u32,
    video_ssrc: u32
}

impl ConnectionData {
    pub fn new (connection: Connection, audio_ssrc: u32, video_ssrc: u32) -> Self {
        ConnectionData { connection, audio_ssrc, video_ssrc }
    }
} 

#[derive(Debug)]
pub struct PeerManager {
    peer_video: DashMap<u32, Peer>,
    peer_audio: DashMap<u32, Peer>,
    peer_connections: DashMap<PublicKey, ConnectionData>,

    pub video_rtp_session: RTPSession,
    pub audio_rtp_session: RTPSession
}

impl PeerManager {
    pub fn new() -> Self {
        let video_ssrc = {
            let mut rng = rand::rng();
            rng.next_u32()
        };

        let video_rtp_session = RTPSession::new(video_ssrc, StreamType::Video);

        let audio_ssrc = {
            let mut rng = rand::rng();
            rng.next_u32()
        };

        let audio_rtp_session = RTPSession::new(audio_ssrc, StreamType::Audio);

        Self {
            peer_video: DashMap::new(),
            peer_audio: DashMap::new(),
            peer_connections: DashMap::new(),
            video_rtp_session,
            audio_rtp_session
        }
    }

    pub fn get_context(&self, ssrc: u32, stream_type: StreamType) -> Option<*mut std::ffi::c_void> {
        let peers = self.get_peer_data(stream_type);

        if let Some(peer) = peers.get(&ssrc) {
            Some(peer.swift_peer_model)
        } else {
            None
        }
    }

    pub fn add_peer_data(
        &self,
        ssrc: u32,
        swift_peer_model: *mut std::ffi::c_void,
        skew_threshold: i32,
        stream_type: StreamType,
    ) -> bool {
        let peers = self.get_peer_data(stream_type);

        if !peers.contains_key(&ssrc) {
            peers.insert(ssrc, Peer::new(swift_peer_model, skew_threshold));
            true
        } else {
            false
        }
    }

    pub fn add_connection(&self, public_key: PublicKey, connection_data: ConnectionData) {
        if !self.peer_connections.contains_key(&public_key) {
            self.peer_connections.insert(public_key, connection_data);
        }
    }

    pub fn remove_peer(&self, public_key: &PublicKey) -> Option<((u32, Peer), (u32, Peer))> {
        let Some((_, connection_data)) = self.peer_connections.remove(public_key) else {
            return None;
        };

        connection_data.connection.close(0u32.into(), b"done");

        let audio = self.peer_audio.remove(&connection_data.audio_ssrc).unwrap();
        let video = self.peer_video.remove(&connection_data.video_ssrc).unwrap();

        Some((audio, video))
    }

    pub fn is_peer_timed_out(&self, ssrc: &u32, stream_type: StreamType) -> bool {
        let peers = self.get_peer_data(stream_type);

        match peers.get(&ssrc) {
            Some(peer) => peer.is_timed_out(),
            None => false,
        }
    }

    pub fn peer_get_min_window(
        &self,
        ssrc: u32,
        difference: u32,
        stream_type: StreamType,
    ) -> Option<u32> {
        let peers = self.get_peer_data(stream_type);

        if let Some(mut found_peer) = peers.get_mut(&ssrc) {
            Some(found_peer.set_and_get_min_window(difference))
        } else {
            None
        }
    }

    pub fn add_playout_node_to_peer(
        &self,
        ssrc: u32,
        playout_buffer_node: PlayoutBufferNode,
        fragment: Fragment,
        difference: u32,
        stream_type: StreamType,
    ) -> Option<i32> {
        let peers = self.get_peer_data(stream_type);

        let Some(mut peer) = peers.get_mut(&ssrc) else {
            return None;
        };

        peer.add_node(playout_buffer_node, fragment);

        Some(peer.skew_calculator.adjust_skew(difference))
    }

    pub fn get_peers(&self) -> Vec<Connection> {
        self.peer_connections
            .iter()
            .map(|entry| entry.value().connection.clone())
            .collect()
    }

    pub fn pop_node(
        &self,
        ssrc: u32,
        timestamp: u32,
        stream_type: StreamType,
    ) -> Option<PlayoutBufferNode> {
        let peers = self.get_peer_data(stream_type);
        let mut peer = peers.get_mut(&ssrc)?;

        let Some(index) = peer
            .playout_buffer
            .iter()
            .position(|x| x.rtp_timestamp == timestamp)
        else {
            return None;
        };

        let node = peer.playout_buffer.remove(index);

        Some(node)
    }

    pub fn determine_stream_type(
        &self, 
        public_key: &PublicKey, 
        ssrc: &u32
    ) -> Option<StreamType> {

        let res = self.peer_connections.get(public_key)?;

        if res.value().audio_ssrc == *ssrc {
           return Some(StreamType::Audio)
        }
        else if res.value().video_ssrc == *ssrc {
            return Some(StreamType::Video)
        }

        None
    } 

    pub fn update_last_sr_timestamp(
        &self,
        ssrc: u32,
        last_sr_timestamp: u32,
        stream_type: StreamType,
    ) {
        let peers = self.get_peer_data(stream_type);
        if let Some(mut peer) = peers.get_mut(&ssrc) {
            peer.update_last_sr_timestamp(last_sr_timestamp);
        }
    }

    pub fn get_reception_reports(&self, stream_type: StreamType) -> Vec<ReceptionReport> {
        let peers = self.get_peer_data(stream_type);

        peers
            .iter()
            .map(|peer| {
                // this isn't even funny omg
                // I don't want to talk about this
                ReceptionReport {
                    reportee_ssrc: *peer.key(),
                    fraction_lost: peer.calculate_fraction_lost(),
                    total_lost: peer.expected_num_packets() - peer.packets_received,
                    extended_sequence_number: peer.max_extended_sequence_num(),
                    jitter: peer.jitter,
                    last_sr_timestamp: peer.last_sr_timestamp,
                    delay_since_last_sr: match peer.delay_since_last_sr {
                        None => 0,
                        Some(time) => {
                            let elapsed = time.elapsed();
                            let seconds = elapsed.as_secs();
                            (seconds * 65536) as u32
                        }
                    },
                }
            })
            .collect()
    }

    fn get_peer_data(&self, stream_type: StreamType) -> &DashMap<u32, Peer> {
        let peers = match stream_type {
            StreamType::Audio => &self.peer_audio,
            StreamType::Video => &self.peer_video,
        };

        &peers
    }
}
