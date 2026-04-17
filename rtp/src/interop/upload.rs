use std::{slice, str::FromStr};

use iroh::{PublicKey, endpoint::presets};
use tokio::{
    fs::File,
    io::{self},
};

use crate::interop::runtime;

#[unsafe(no_mangle)]
pub extern "C" fn swift_upload(
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
    println!("{file_path} ::: {endpoint_id}");

    runtime().spawn(async move {
        if let Err(e) = upload(file_path, endpoint_id).await {
            eprint!("{e}");
        }
    });
}

async fn upload(file_path: String, endpoint_id: String) -> io::Result<()> {
    // TODO: persist this
    let secret_key = iroh::SecretKey::generate();
    let Ok(endpoint) = iroh::Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .bind()
        .await
    else {
        return Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "Could not bind endpoint",
        ));
    };

    let remote: PublicKey = PublicKey::from_str(&endpoint_id.trim()).map_err(|e| {
        io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("Could not convert input to hash: {}", e),
        )
    })?;

    let connection = endpoint.connect(remote, b"fun").await.map_err(|e| {
        io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("Could not connect to remote endpoint: {}", e),
        )
    })?;

    let (mut send, mut recv) = connection.open_bi().await.map_err(|e| {
        io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("Could not open connection to remote endpoint: {}", e),
        )
    })?;

    let mut video = File::open(file_path).await?;

    tokio::io::copy(&mut video, &mut send).await?;

    send.finish()?;

    // TODO: store the hash or something
    let res = recv.read_to_end(128).await.unwrap();

    connection.close(0u32.into(), b"all done!");

    Ok(())
}
