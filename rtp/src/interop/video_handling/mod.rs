use std::{io, slice};

use iroh::SecretKey;
use serde::{Deserialize, Serialize};

use crate::interop::{
    runtime,
    video_handling::upload::{get_everything, run_router, upload_handler},
};

pub mod download;
pub mod upload;

const KEY_PATH: &str = "key.txt";

#[unsafe(no_mangle)]
pub extern "C" fn rust_setup_docs() {
    println!("Starting setup: ");

    runtime().spawn(async move {
        if let Err(e) = run_router().await {
            eprintln!("Router failure: {}", e);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_upload(
    file_path: *const u8,
    file_path_len: usize,
    endpoint_id: *const u8,
    endpoint_id_length: usize,
) {
    let file_path = unsafe { slice::from_raw_parts(file_path, file_path_len) };
    let endpoint_id = unsafe { slice::from_raw_parts(endpoint_id, endpoint_id_length) };

    let Ok(file_path) = str::from_utf8(file_path).map(|s| s.to_string()) else {
        return;
    };

    let Ok(endpoint_id) = str::from_utf8(endpoint_id).map(|s| s.to_string()) else {
        return;
    };
    println!("File path: {file_path}\n, sent to {endpoint_id}");

    runtime().spawn(async move {
        if let Err(e) = upload_handler(file_path, endpoint_id).await {
            eprint!("{e}");
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_change_permissions(list_ptr: *const u8, ptr_length: usize) {
    let slice = unsafe { slice::from_raw_parts(list_ptr, ptr_length) };
    let slice = slice.to_owned(); // we're gonna copy to avoid messy memory management with swift.

    let authorized_users: AuthorizedUsers = match serde_json::from_slice(&slice) {
        Ok(authoirzed_users) => authoirzed_users,
        Err(e) => {
            eprintln!("Serialization error {e}");
            return;
        }
    };

    
}

pub type UpdateListCallback =
    extern "C" fn(context: *mut std::ffi::c_void, ptr: *const u8, count: usize);

pub struct UpdateListCallbackContainer {
    context: *mut std::ffi::c_void,
    update_list_callback: UpdateListCallback,
}

unsafe impl Send for UpdateListCallbackContainer {}

#[unsafe(no_mangle)]
pub extern "C" fn rust_get_remote_videos(
    context: *mut std::ffi::c_void,
    update_list_callback: UpdateListCallback,
) {
    let container = UpdateListCallbackContainer {
        context,
        update_list_callback,
    };

    runtime().spawn(async move {
        if let Err(e) = get_everything(container).await {
            eprintln!("failed to get access lists: {}", e);
        };
    });
}

async fn get_key() -> io::Result<SecretKey> {
    if tokio::fs::try_exists(KEY_PATH).await? {
        let bytes = tokio::fs::read(KEY_PATH).await?;
        match bytes[..32].try_into() {
            Ok(bytes) => Ok(iroh::SecretKey::from_bytes(bytes)),
            Err(_) => store_key().await,
        }
    } else {
        store_key().await
    }
}

async fn store_key() -> io::Result<SecretKey> {
    let key = SecretKey::generate();

    tokio::fs::write(KEY_PATH, key.to_bytes()).await?;

    Ok(key)
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub struct AuthorizedUsers {
    pub namespace_id: String,
    pub authorized_users: Vec<String> 
}