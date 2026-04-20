use std::{ffi::c_void, slice, str::FromStr};
use iroh::{Endpoint, EndpointId, endpoint::presets};

use crate::interop::runtime;

unsafe extern "C" {
    fn swift_receive_video (context: *mut c_void);

    fn swift_receive_hashes (
        context: *mut c_void, 
        hashes: *const u8,
        count: usize
    );
}

struct DownloadCallback {
    endpoint_id: String,
    hash_sequence: String,
    context: *mut c_void
}

unsafe impl Send for DownloadCallback {}

#[unsafe(no_mangle)]
pub extern "C" fn swift_download(
    hash_sequence: *const u8, 
    hash_sequence_length: usize,
    endpoint: *const u8,
    endpoint_length: usize,
    context: *mut c_void
){
    if hash_sequence.is_null() || endpoint.is_null() {
        return;
    }

    let hash_sequence = unsafe { slice::from_raw_parts(hash_sequence, hash_sequence_length) };

    let Ok(hash_sequence) = str::from_utf8(hash_sequence).map(|s| s.to_string()) else {
        return;
    };

    let endpoint = unsafe { slice::from_raw_parts(endpoint, endpoint_length) };
    let Ok(endpoint_id) = str::from_utf8(endpoint).map(|s| s.to_string()) else {
        return;
    };
    
    let download_callback = DownloadCallback {
        endpoint_id, 
        hash_sequence, 
        context
    };

    runtime().spawn(async move {
        if let Err(e) = query_hash_sequence(download_callback).await {
            eprintln!("Couldn't query endpoint for hash: {}", e)
        };
    });
}

async fn query_hash_sequence(download_callback: DownloadCallback) -> anyhow::Result<()> {
    let client_endpoint = Endpoint::bind(presets::N0).await?;
    let endpoint_id = EndpointId::from_str(download_callback.endpoint_id.trim()).unwrap();

    let conn = client_endpoint.connect(endpoint_id, b"query").await?;

    let (mut send, mut recv) = conn.open_bi().await?;

    send.write_all(download_callback.hash_sequence.as_bytes()).await?;
    send.finish()?;

    let mut buff = [0u8; 32];
    let mut hashes: Vec<u8> = Vec::new();

    println!("Connection opened");
    loop {
        match recv.read_exact(&mut buff).await {
            Err(iroh::endpoint::ReadExactError::FinishedEarly(_)) => break,
            Err(err) => return Err(err.into()),
            Ok(_) => {}
        };
        hashes.extend_from_slice(&buff);
    }
    conn.close(0u32.into(), b"done");

    unsafe {
        swift_receive_hashes(download_callback.context, hashes.as_ptr(), hashes.len());
    }

    Ok(())
}