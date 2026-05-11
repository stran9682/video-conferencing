pub mod delay_calculator;
pub mod peer_manager;

use std::{ffi::c_void};

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