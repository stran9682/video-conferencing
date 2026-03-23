use std::{
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use bytes::Bytes;
use quinn::{
    ClientConfig, Connection, Endpoint, SendDatagramError, ServerConfig,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivatePkcs8KeyDer},
    },
};

#[allow(unused)]
async fn send_unreliable(connection: &Connection) -> Result<(), SendDatagramError> {
    connection.send_datagram(Bytes::from(&b"test"[..]))?;
    Ok(())
}

#[allow(unused)]
async fn receive_datagram(connection: &Connection) {
    if let Ok(received_bytes) = connection.read_datagram().await {
        // Because it is a unidirectional stream, we can only receive not send back.
        println!(
            "request from {:?}: {:?}",
            connection.remote_address(),
            received_bytes
        );
    }
}

fn make_server_endpoint(bind_addr: SocketAddr) -> io::Result<(Endpoint, CertificateDer<'static>)> {
    let (server_config, server_cert) = configure_server().map_err(|e| {
        io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("Failed to create QUIC endpoint: {}", e),
        )
    })?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

fn configure_server() -> Result<(ServerConfig, CertificateDer<'static>), rustls::Error> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert);
    let priv_key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());

    let mut server_config =
        ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into())?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok((server_config, cert_der))
}

fn make_client_endpoint(
    bind_addr: SocketAddr,
    server_certs: &[&[u8]],
) -> Result<Endpoint, Box<dyn Error + Send + Sync + 'static>> {
    let client_cfg = configure_client(server_certs)?;
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

fn configure_client(server_certs: &[&[u8]]) -> Result<ClientConfig, rustls::Error> {
    let mut certs = rustls::RootCertStore::empty();
    for cert in server_certs {
        let cert = CertificateDer::from(*cert);
        certs.add(cert)?;
    }

    Ok(ClientConfig::with_root_certificates(Arc::new(certs))
        .map_err(|_| rustls::Error::HandshakeNotComplete)?)
}

fn run_server(addr: SocketAddr) -> io::Result<CertificateDer<'static>> {
    let (endpoint, server_cert) = make_server_endpoint(addr)?;
    // accept a single connection
    tokio::spawn(async move {
        let connection = endpoint.accept().await.unwrap().await.unwrap();
        println!(
            "[server] incoming connection: addr={}",
            connection.remote_address()
        );

        receive_datagram(&connection).await;
    });

    Ok(server_cert)
}

/// Attempt QUIC connection with the given server address.
async fn run_client(endpoint: &Endpoint, server_addr: SocketAddr) {
    let connect = endpoint.connect(server_addr, "localhost").unwrap();
    let connection = connect.await.unwrap();
    match send_unreliable(&connection).await {
        Ok(_) => (),
        Err(_) => println!("Disaster!"),
    };

    println!("[client] connected: addr={}", connection.remote_address());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);
    let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5001);
    let addr3 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5002);
    let server1_cert = run_server(addr1)?;
    let server2_cert = run_server(addr2)?;
    let server3_cert = run_server(addr3)?;

    let client = make_client_endpoint(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        &[&server1_cert, &server2_cert, &server3_cert],
    )?;

    // connect to multiple endpoints using the same socket/endpoint
    tokio::join!(
        run_client(&client, addr1),
        run_client(&client, addr2),
        run_client(&client, addr3),
    );

    // Make sure the server has a chance to clean up
    client.wait_idle().await;

    Ok(())
}
