use std::{io::Error, str::FromStr, sync::OnceLock};

use anyhow::Ok;
use iroh::{PublicKey, endpoint::presets, protocol::Router};
use iroh_blobs::{BlobsProtocol, store::fs::FsStore};
use iroh_docs::{DocTicket, NamespaceId, api::protocol::ShareMode, protocol::Docs, store::Query};
use iroh_gossip::Gossip;
use n0_future::StreamExt;
use tokio::{
    fs::File,
    io::{self},
};

use crate::interop::video_handling::{AuthorizedUsers, GetListCallbackContainer, get_key};

// I LOVE STATICS WE ALL SAY IN UNISON!
static ROUTER: OnceLock<Router> = OnceLock::new();

pub struct UploadManager {
    docs: Docs,
    blobs: FsStore,
}

impl UploadManager {
    pub fn new(docs: &Docs, blobs: &FsStore) -> Self {
        Self {
            docs: docs.clone(),
            blobs: blobs.clone(),
        }
    }

    pub async fn upload_handler(
        &self,
        file_path: String,
        endpoint_id: String,
    ) -> anyhow::Result<()> {
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
        let author = self.docs.author_create().await?; // TODO adjust this
        let doc = self.docs.import(ticket).await?;

        let entry = AuthorizedUsers {
            namespace_id: doc.id().to_string(),
            authorized_users: vec![endpoint.id().to_string()],
        };
        let entry = serde_json::to_vec(&entry)?;

        doc.set_bytes(author, "accesslist", entry).await?;

        connection.close(0u32.into(), b"all done!");

        Ok(())
    }

    pub async fn get_documents(&self, container: GetListCallbackContainer) -> anyhow::Result<()> {
        let mut res = self.docs.list().await?;

        while let Some(entry) = res.next().await {
            println!("Have a document");

            let (namespace, _) = entry?;

            let doc = self.docs.open(namespace).await?.ok_or(Error::new(
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

                let bytes = self.blobs.get_bytes(hash).await?;

                println!("Sending to swift");

                (container.update_list_callback)(container.context, bytes.as_ptr(), bytes.len())
            }
        }

        println!("Exiting");

        Ok(())
    }

    pub async fn update_access_control_list_for_doc(
        &self,
        authorized_users: AuthorizedUsers,
    ) -> anyhow::Result<()> {
        let namespace_id = NamespaceId::from_str(&authorized_users.namespace_id)?;
        let doc = self.docs.open(namespace_id).await?.ok_or(Error::new(
            io::ErrorKind::NotFound,
            "Could not find document associated with namespace",
        ))?;

        let entry = serde_json::to_vec(&authorized_users)?;

        doc.set_bytes(self.docs.author_default().await?, "accesslist", entry)
            .await?;

        Ok(())
    }

    pub async fn get_doc_ticket(&self, namespace_id: NamespaceId) -> anyhow::Result<String> {
        let doc = self.docs.open(namespace_id).await?.ok_or(Error::new(
            io::ErrorKind::NotFound,
            "Could not find document associated with namespace",
        ))?;

        let ticket = doc.share(ShareMode::Write, Default::default()).await?;

        Ok(ticket.to_string())
    }
}

pub async fn run_router() -> anyhow::Result<*mut UploadManager> {
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

    let mut res = docs.list().await?;
    while let Some(entry) = res.next().await {
        let (namespace_id, _) = entry?;

        let doc = docs.open(namespace_id).await?.ok_or(Error::new(
            io::ErrorKind::InvalidFilename,
            "Couldn't open document",
        ))?;

        let ticket = doc.share(ShareMode::Write, Default::default()).await?;
        let ticket_str = ticket.to_string();

        // no way you can do this.
        // bahah using your document to rejoin
        println!("Doc ticket on startup: {}", ticket.to_string());
        docs.import(DocTicket::from_str(&ticket_str)?).await?;
    }

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

    let upload_manager = UploadManager::new(&docs, &blobs);

    Ok(Box::into_raw(Box::new(upload_manager)))
}
