// Connection serving adapted from axum-server 0.7.3 (https_axum_server/SOURCE.json).
use axum::{Extension, Router, extract::ConnectInfo};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use std::{io, net, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinSet,
    time,
};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

pub use tokio_rustls::rustls::ServerConfig;

pub async fn accept_tls(
    config: Arc<ServerConfig>,
    stream: TcpStream,
) -> io::Result<TlsStream<TcpStream>> {
    stream.set_nodelay(true)?;
    time::timeout(
        Duration::from_secs(10),
        TlsAcceptor::from(config).accept(stream),
    )
    .await?
}

pub async fn serve(
    listener: net::TcpListener,
    router: Router,
    config: ServerConfig,
    mut stop: watch::Receiver<bool>,
) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(listener)?;
    let config = Arc::new(config);
    let builder = Arc::new(Builder::new(TokioExecutor::new()));
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = async { let _ = stop.wait_for(|&stopped| stopped).await; } => break,
            _ = connections.join_next(), if !connections.is_empty() => {}
            (stream, address) = accept(&listener) => {
                let config = config.clone();
                let builder = builder.clone();
                let router = router.clone().layer(Extension(ConnectInfo(address)));
                let mut stop = stop.clone();
                connections.spawn(async move {
                    let Ok(stream) = accept_tls(config, stream).await else {
                        return;
                    };
                    let service = TowerToHyperService::new(router);
                    let connection = builder.serve_connection_with_upgrades(TokioIo::new(stream), service);
                    tokio::pin!(connection);
                    tokio::select! {
                        biased;
                        _ = async { let _ = stop.wait_for(|&stopped| stopped).await; } => {
                            connection.as_mut().graceful_shutdown();
                            let _ = connection.await;
                        }
                        _ = &mut connection => {}
                    }
                });
            }
        }
    }

    drop(listener);
    if time::timeout(Duration::from_secs(1), async {
        while connections.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        connections.shutdown().await;
    }
    Ok(())
}

async fn accept(listener: &TcpListener) -> (TcpStream, net::SocketAddr) {
    loop {
        match listener.accept().await {
            Ok(connection) => return connection,
            Err(_) => time::sleep(Duration::from_millis(50)).await,
        }
    }
}
