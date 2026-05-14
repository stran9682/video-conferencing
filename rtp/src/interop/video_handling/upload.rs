use std::{io::Error, str::FromStr, sync::OnceLock};

use iroh::{PublicKey, endpoint::presets, protocol::Router};
use iroh_blobs::{BlobsProtocol, store::fs::FsStore};
use iroh_docs::{DocTicket, protocol::Docs};
use iroh_gossip::Gossip;
use tokio::{
    fs::File,
    io::{self},
};

use crate::interop::video_handling::{AuthorizedUsers, get_key};

static ROUTER: OnceLock<Router> = OnceLock::new();
static DOCS: OnceLock<Docs> = OnceLock::new();

pub async fn run_router() -> anyhow::Result<()> {
    let secret_key = get_key().await?;

    println!("Got the key");

    let endpoint = iroh::Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .bind()
        .await?;

    println!("Created the endpoint");

    let blobs = FsStore::load("./blobs").await?;
    let gossip = Gossip::builder().spawn(endpoint.clone());
    let docs = Docs::persistent("./blobs".into())
        .spawn(endpoint.clone(), (*blobs).clone(), gossip.clone())
        .await?;

    let author_id = docs.author_create().await?;
    docs.author_set_default(author_id).await?;

    println!("Started the dependencies");

    let router = Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, BlobsProtocol::new(&blobs, None))
        .accept(iroh_gossip::ALPN, gossip)
        .accept(iroh_docs::ALPN, docs.clone())
        .spawn();

    println!("Started the router");

    if let Err(e) = ROUTER.set(router) {
        eprintln!("Router already created, shutting down");
        e.shutdown().await?;
    }

    println!("Saved the router");

    if let Err(_) = DOCS.set(docs) {
        eprintln!("Docs already created");
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Docs already created").into());
    }

    println!("Saved the docs");

    Ok(())
}

pub async fn upload_handler(file_path: String, endpoint_id: String) -> anyhow::Result<()> {
    let docs = DOCS.get().ok_or(Error::new(
        io::ErrorKind::NotFound,
        "Docs wasn't initialized, have you started the router?",
    ))?;

    let secret_key = get_key().await?;

    let endpoint = iroh::Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .bind()
        .await?;

    let remote: PublicKey = PublicKey::from_str(endpoint_id.trim())?;

    let connection = endpoint.connect(remote, b"fun").await?;

    let (mut send, mut recv) = connection.open_bi().await?;

    let mut video = File::open(file_path).await?;

    tokio::io::copy(&mut video, &mut send).await?;

    send.finish()?;

    // receiving the doc ticket
    let bytes = recv.read_to_end(256).await?;
    let ticket_str = str::from_utf8(&bytes)?;
    println!("Received a ticket: {}", ticket_str);
    let ticket = DocTicket::from_str(ticket_str.trim())?;

    // receiving the tag of the video
    let mut recv = connection.accept_uni().await?;
    let bytes = recv.read_to_end(256).await?;
    let tag = String::from_utf8(bytes)?;
    println!("Doc ID : # Clips: {}", tag);

    // Now save the document
    let doc = docs.import(ticket).await?;

    // add ourselves to the list of authorized users.
    let entry = AuthorizedUsers {
        authorized_users: vec![endpoint.id().to_string()],
    };
    let entry = serde_json::to_vec(&entry)?;

    doc.set_bytes(docs.author_default().await?, tag, entry)
        .await?;

    connection.close(0u32.into(), b"all done!");

    Ok(())
}
