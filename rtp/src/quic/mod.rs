use std::{io, net::SocketAddr, sync::Arc};

use quinn::{
    Endpoint, ServerConfig,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivatePkcs8KeyDer},
    },
};

pub fn make_server_endpoint(
    bind_addr: SocketAddr,
    ssrc: &u32,
) -> io::Result<(Endpoint, CertificateDer<'static>)> {
    let (server_config, server_cert) = configure_server(ssrc).map_err(|e| {
        io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("Failed to create QUIC endpoint: {}", e),
        )
    })?;

    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

fn configure_server(ssrc: &u32) -> Result<(ServerConfig, CertificateDer<'static>), rustls::Error> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert);
    let priv_key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());

    let mut server_config =
        ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into())?;

    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok((server_config, cert_der))
}
