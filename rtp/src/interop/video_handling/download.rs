use iroh::{EndpointId, endpoint::presets};
use std::{ffi::c_void, slice, str::FromStr};
use tokio::fs::{File, create_dir_all};
use uuid::Uuid;

use crate::interop::{runtime, video_handling::get_key};

unsafe extern "C" {
    fn swift_receive_video(context: *mut c_void, path: *const u8);

    fn swift_release_pointer(context: *mut c_void);
}

struct DownloadCallback {
    endpoint_id: String,
    tag_str: String,
    context: *mut c_void,
}

unsafe impl Send for DownloadCallback {}
unsafe impl Sync for DownloadCallback {}

#[unsafe(no_mangle)]
pub extern "C" fn swift_download(
    tag: *const u8,
    tag_length: usize,
    endpoint: *const u8,
    endpoint_length: usize,
    context: *mut c_void,
) {
    if tag.is_null() || endpoint.is_null() {
        return;
    }

    let tag_slice = unsafe { slice::from_raw_parts(tag, tag_length) };
    let Ok(tag_str) = str::from_utf8(tag_slice).map(|s| s.to_string()) else {
        eprintln!("Invalid tag string");
        return;
    };

    let endpoint = unsafe { slice::from_raw_parts(endpoint, endpoint_length) };
    let Ok(endpoint_id) = str::from_utf8(endpoint).map(|s| s.to_string()) else {
        eprintln!("Invalid endpoint string");
        return;
    };

    let download_callback = DownloadCallback {
        endpoint_id,
        tag_str,
        context,
    };

    runtime().spawn(async move {
        if let Err(e) = query_hash(&download_callback).await {
            eprintln!("Couldn't query endpoint for hash: {}", e);

            // TODO: if connection fails, release the retain on context swift side, else another memory leak!
            unsafe {
                swift_release_pointer(download_callback.context);
            };
        };
    });
}

async fn query_hash(download_callback: &DownloadCallback) -> anyhow::Result<()> {
    // TODO: use your persisted secret key to bind
    let secret_key = get_key().await?;

    let client_endpoint = iroh::Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .bind()
        .await?;

    let endpoint_id = EndpointId::from_str(download_callback.endpoint_id.trim()).unwrap();

    let conn = client_endpoint.connect(endpoint_id, b"query").await?;

    let (mut send, mut recv) = conn.open_bi().await?;

    send.write_all(download_callback.tag_str.as_bytes()).await?;
    send.finish()?;

    let path = format!("clips/{}.mp4", Uuid::new_v4());
    if let Some(parent) = std::path::Path::new(&format!("tmp/{}", path)).parent() {
        create_dir_all(parent).await?;
    }

    let mut temp_file = File::create(format!("tmp/{}", path)).await?;
    tokio::io::copy(&mut recv, &mut temp_file).await?;
    println!("{}", download_callback.tag_str);
    unsafe {
        swift_receive_video(download_callback.context, path.as_ptr());
    }

    conn.close(0u32.into(), b"all done!");

    Ok(())
}
