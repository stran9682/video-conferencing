use std::{ffi::c_void, slice, str::FromStr};
use iroh::{Endpoint, EndpointId, endpoint::presets};
use iroh_blobs::Hash;
use tokio::fs::{File, create_dir_all};
use uuid::Uuid;

use crate::interop::runtime;

unsafe extern "C" {
    fn swift_receive_video (
        context: *mut c_void,
        path: *const u8
    );

    fn swift_receive_hashes (
        context: *mut c_void, 
        hashes: *const u8,
        count: usize
    );
}

struct DownloadCallback {
    endpoint_id: String,
    hash_sequence: String,
    context: *mut c_void,
    is_hash_query: bool
}

unsafe impl Send for DownloadCallback {}

#[unsafe(no_mangle)]
pub extern "C" fn swift_download(
    hash_sequence: *const u8, 
    hash_sequence_length: usize,
    endpoint: *const u8,
    endpoint_length: usize,
    context: *mut c_void,
    is_hash_query: bool
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
        context,
        is_hash_query
    };

    runtime().spawn(async move {
        if let Err(e) = query_hash(download_callback).await {
            eprintln!("Couldn't query endpoint for hash: {}", e)
        };
    });
}

async fn query_hash(download_callback: DownloadCallback) -> anyhow::Result<()> {
    let client_endpoint = Endpoint::bind(presets::N0).await?;
    let endpoint_id = EndpointId::from_str(download_callback.endpoint_id.trim()).unwrap();

    let conn = client_endpoint.connect(endpoint_id, b"query").await?;

    let (mut send, mut recv) = conn.open_bi().await?;

    send.write_all(download_callback.hash_sequence.as_bytes()).await?;
    send.finish()?;

    if download_callback.is_hash_query {

        let mut buff = [0u8; 32];
        let mut hashes: Vec<u8> = Vec::new();

        println!("Connection opened");
        loop {
            match recv.read_exact(&mut buff).await {
                Err(iroh::endpoint::ReadExactError::FinishedEarly(_)) => break,
                Err(err) => return Err(err.into()),
                Ok(_) => {}
            };
            let hash = Hash::from_bytes(buff);

            hashes.extend_from_slice(hash.to_hex().as_bytes());
        }
        conn.close(0u32.into(), b"done");

        unsafe {
            swift_receive_hashes(download_callback.context, hashes.as_ptr(), hashes.len());
        }
    }
    else {
        let path = format!("clips/{}.mp4", Uuid::new_v4());
        if let Some(parent) = std::path::Path::new(&format!("tmp/{}", path)).parent() {
            create_dir_all(parent).await?;
        }

        let mut temp_file = File::create(format!("tmp/{}", path)).await?;
        tokio::io::copy(&mut recv, &mut temp_file).await?;

        unsafe {
            swift_receive_video(
                download_callback.context,
                path.as_ptr()
            );
        }
    }

    Ok(())
}