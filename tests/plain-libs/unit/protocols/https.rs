use super::*;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, pem::PemObject};
use socket2::SockRef;

#[tokio::test]
async fn only_multiplexed_tls_connections_bound_the_send_queue() {
    let (cert, key) = crate::tls::generate_pem(&["localhost".into()]).unwrap();
    let cert = CertificateDer::from_pem_slice(&cert).unwrap();
    let key = PrivateKeyDer::from_pem_slice(&key).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut server = ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], key)
        .unwrap();
    server.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let server = Arc::new(server);
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert).unwrap();
    let client = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    for protocol in [b"http/1.1".as_slice(), b"h2"] {
        let mut client = client.clone();
        client.alpn_protocols = vec![protocol.to_vec()];
        let connect = async {
            let stream = TcpStream::connect(listener.local_addr().unwrap())
                .await
                .unwrap();
            tokio_rustls::TlsConnector::from(Arc::new(client))
                .connect(ServerName::try_from("localhost").unwrap(), stream)
                .await
                .unwrap()
        };
        let accept = async {
            let (stream, _) = listener.accept().await.unwrap();
            let sock = SockRef::from(&stream);
            sock.set_send_buffer_size(256 * 1024).unwrap();
            let inherited = sock.send_buffer_size().unwrap();
            let stream = accept_tls(server.clone(), stream).await.unwrap();
            assert_eq!(stream.get_ref().1.alpn_protocol(), Some(protocol));
            let sock = SockRef::from(stream.get_ref().0);
            let actual = sock.send_buffer_size().unwrap();
            if cfg!(any(target_os = "linux", target_os = "android")) && protocol == b"h2" {
                sock.set_send_buffer_size(64 * 1024).unwrap();
                assert_eq!(actual, sock.send_buffer_size().unwrap());
            } else {
                assert_eq!(actual, inherited);
            }
        };
        tokio::join!(connect, accept);
    }
}
