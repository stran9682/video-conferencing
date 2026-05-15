use std::{io::Error, str::FromStr, sync::OnceLock};

use iroh::{PublicKey, endpoint::presets, protocol::Router};
use iroh_blobs::{BlobsProtocol, store::fs::FsStore};
use iroh_docs::{DocTicket, protocol::Docs, store::Query};
use iroh_gossip::Gossip;
use tokio::{
    fs::File,
    io::{self},
};
use n0_future::StreamExt;

use crate::interop::video_handling::{AuthorizedUsers, UpdateListCallbackContainer, get_key};

// I LOVE STATICS WE ALL SAY IN UNISON!
static ROUTER: OnceLock<Router> = OnceLock::new();
static DOCS: OnceLock<Docs> = OnceLock::new();
static BLOBS: OnceLock<FsStore> = OnceLock::new();

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


    if let Err(_) = BLOBS.set(blobs) {
        eprintln!("Blobs already created");
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Blob already created").into());
    }

    println!("Saved the blobs");

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

    let mut video = File::open(&file_path).await?;

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
    let author = docs.author_create().await?; // TODO adjust this
    let doc = docs.import(ticket).await?;

    let entry = AuthorizedUsers {
        authorized_users: vec![endpoint.id().to_string()],
    };
    let entry = serde_json::to_vec(&entry)?;

    doc.set_bytes(author, "accesslist", entry)
        .await?;

    connection.close(0u32.into(), b"all done!");

    Ok(())
}

// Tough times call for tough solutions
pub async fn get_everything(container: UpdateListCallbackContainer) -> anyhow::Result<()> {
    let docs = DOCS.get().ok_or(Error::new(
        io::ErrorKind::NotFound,
        "Docs wasn't initialized, have you started the router?",
    ))?;

    let blobs = BLOBS.get().ok_or(Error::new(
        io::ErrorKind::NotFound,
        "Blobs wasn't initialized, have you started the router?",
    ))?;

    println!("Everything good to start");

    let mut res = docs.list().await?;

    while let Some(entry) = res.next().await {

        println!("Have a document");

        let (namespace, _) = entry?;

        let doc = docs.open(namespace).await?.ok_or(Error::new(
            io::ErrorKind::InvalidFilename,
            "Couldn't open document",
        ))?;

        println!("Opening document");

        if let Some(doc_entry) = doc
            .get_one(Query::single_latest_per_key().key_exact("accesslist"))
            .await?
        {
            println!("Getting access list entry");

            let hash = doc_entry.content_hash();

            let bytes = blobs.get_bytes(hash).await?;

            println!("Sending to swift");

            (container.update_list_callback)(container.context, bytes.as_ptr(), bytes.len())
        }
    }

    println!("Exiting");

    Ok(())
}
