use serde::{Deserialize, Serialize};
use std::ffi::c_void;

use crate::interop::StreamType;

// TODO: update addr to use SSRC instead of address
unsafe extern "C" {
    pub fn swift_receive_pps_sps(
        context: *mut c_void,
        pps: *const u8,
        pps_length: usize,
        sps: *const u8,
        sps_length: usize,
        ssrc: u32,
    ) -> *mut c_void;

    pub fn swift_receive_audio_config(
        audio_manager_context: *mut c_void,
        sample_rate: f64,
        channels: u32,
        ssrc: u32,
    ) -> *mut c_void;

    pub fn swift_remove_audio_peer(
        audio_manager_context: *mut c_void,
        ssrc: u32,
        participant_context: *mut c_void,
    );

    pub fn swift_remove_video_peer(
        ssrc: u32,
        video_manager_context: *mut c_void,
        peer_context: *mut c_void,
    );
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
enum StreamTypeWithArgs {
    Video { pps: Vec<u8>, sps: Vec<u8> },
    Audio { sample_rate: f64, channels: u32 },
}

impl StreamTypeWithArgs {
    pub fn to_stream_type(&self) -> StreamType {
        match self {
            StreamTypeWithArgs::Audio {
                sample_rate: _,
                channels: _,
            } => StreamType::Audio,
            StreamTypeWithArgs::Video { pps: _, sps: _ } => StreamType::Video,
        }
    }
}

pub struct OpusArgs {
    pub sample_rate: f64,
    pub channels: u32,
}
