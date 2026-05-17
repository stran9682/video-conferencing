use std::{ffi::CString, io, ptr, slice, str::FromStr};

use iroh::SecretKey;
use iroh_docs::NamespaceId;
use serde::{Deserialize, Serialize};

use crate::interop::{
    runtime,
    video_handling::upload::{UploadManager, run_router},
};

pub mod download;
pub mod upload;

const KEY_PATH: &str = "key.txt";

pub type UpdateListCallback =
    extern "C" fn(context: *mut std::ffi::c_void, ptr: *const u8, count: usize);

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub struct AuthorizedUsers {
    pub namespace_id: String,
    pub authorized_users: Vec<String> 
}

pub struct GetListCallbackContainer {
    context: *mut std::ffi::c_void,
    update_list_callback: UpdateListCallback,
}

unsafe impl Send for GetListCallbackContainer {}

#[unsafe(no_mangle)]
pub extern "C" fn rust_setup_docs() -> *mut UploadManager {
    println!("Starting setup: ");

    let ptr = runtime().block_on(async {
        run_router().await
    });

    ptr.unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_deallocate_uploadmanager(upload_manager_ptr: *mut UploadManager) {
    if !upload_manager_ptr.is_null() {
        drop(unsafe { Box::from_raw(upload_manager_ptr) }); // Take ownership back and drop it
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_upload(
    upload_manager_ptr: *mut UploadManager,
    file_path: *const u8,
    file_path_len: usize,
    endpoint_id: *const u8,
    endpoint_id_length: usize,
) -> bool {
    if upload_manager_ptr.is_null() { return false; }

    let upload_manager = unsafe { &*upload_manager_ptr };

    let file_path = unsafe { slice::from_raw_parts(file_path, file_path_len) };
    let endpoint_id = unsafe { slice::from_raw_parts(endpoint_id, endpoint_id_length) };

    let Ok(file_path) = str::from_utf8(file_path).map(|s| s.to_string()) else {
        return false;
    };

    let Ok(endpoint_id) = str::from_utf8(endpoint_id).map(|s| s.to_string()) else {
        return false;
    };
    println!("File path: {file_path}\n, sent to {endpoint_id}");

    match runtime().block_on(async {
        upload_manager.upload_handler(file_path, endpoint_id).await
    }) {
        Ok(()) => true,
        Err(_) => false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_change_permissions(
    upload_manager_ptr: *mut UploadManager, 
    list_ptr: *const u8, 
    ptr_length: usize
) -> bool {
    if upload_manager_ptr.is_null() { return false; }

    let upload_manager = unsafe { &*upload_manager_ptr };

    let slice = unsafe { slice::from_raw_parts(list_ptr, ptr_length) };
    let slice = slice.to_owned(); // we're gonna copy to avoid messy memory management with swift.

    let authorized_users: AuthorizedUsers = match serde_json::from_slice(&slice) {
        Ok(authoirzed_users) => authoirzed_users,
        Err(e) => {
            eprintln!("Serialization error {e}");
            return false;
        }
    };

    match runtime().block_on(async {
        upload_manager.update_access_control_list_for_doc(authorized_users).await
    }) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("Failed to update document {e}");
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_get_shared_videos(
    upload_manager_ptr: *mut UploadManager,
    context: *mut std::ffi::c_void,
    update_list_callback: UpdateListCallback,
) {
    if upload_manager_ptr.is_null() { return; }

    let upload_manager = unsafe { &*upload_manager_ptr };

    let container = GetListCallbackContainer {
        context,
        update_list_callback,
    };

    runtime().spawn(async move {
        if let Err(e) = upload_manager.get_documents(container).await {
            eprintln!("failed to get access lists: {}", e);
        };
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_get_doc_ticket(
    upload_manager_ptr: *mut UploadManager,
    namespace_id_ptr: *const u8,
    ptr_length: usize,
    buffer: *mut i8
) -> bool {
    if upload_manager_ptr.is_null() || namespace_id_ptr.is_null() { return true }

    let upload_manager = unsafe { &*upload_manager_ptr };

    let namespace_id = unsafe { slice::from_raw_parts(namespace_id_ptr, ptr_length) };
    let Ok(namespace_id) = str::from_utf8(namespace_id) else {
        return false
    };

    let Ok(namespace_id) = NamespaceId::from_str(namespace_id) else {
        return false
    };

    match runtime().block_on(upload_manager.get_doc_ticket(namespace_id)) {
        Ok(ticket) => {
            println!("Rust side: {}", ticket);

            let c_string = CString::new(ticket).unwrap();
            unsafe {
                std::ptr::copy_nonoverlapping(
                    c_string.as_ptr() as *const i8,
                    buffer,
                    c_string.count_bytes(),
                );
            }

            true
        }
        Err(_) =>  false
    }
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