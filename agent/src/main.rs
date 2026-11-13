use anyhow::Result;
use rustls::pki_types::ServerName;
use shadowtunnel_protocol::{decode, encode, Message, MAX_DATAGRAM, MAX_FRAME};
use std::{
    collections::HashMap, env, fs::File, io::BufReader, net::IpAddr, sync::Arc, time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
    sync::{mpsc, Mutex},
    time::sleep,
};
use tokio_rustls::{rustls, TlsConnector};

trait AsyncStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static> AsyncStream for T {}

async fn connect_gateway(
    endpoint: String,
    tls: bool,
    ca: Option<String>,
    server_name: Option<String>,
) -> Result<Box<dyn AsyncStream>> {
    let stream = TcpStream::connect(endpoint).await?;
    if !tls {
        return Ok(Box::new(stream));
    }
    let mut roots = rustls::RootCertStore::empty();
    let mut ca_reader = BufReader::new(File::open(
        ca.ok_or_else(|| anyhow::anyhow!("SHADOW_TLS_CA required"))?,
    )?);
    let certs = rustls_pemfile::certs(&mut ca_reader);
    for cert in certs {
        roots.add(cert?)?;
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from(
        server_name.ok_or_else(|| anyhow::anyhow!("SHADOW_TLS_SERVER_NAME required"))?,
    )?
    .to_owned();
    Ok(Box::new(
        TlsConnector::from(Arc::new(config))
            .connect(name, stream)
            .await?,
    ))
}
fn valid_type(kind: &str) -> Result<()> {
    if kind != "java" && kind != "bedrock" {
        anyhow::bail!("unsupported Minecraft type: {kind}")
    }
    Ok(())
}

fn valid_transport(transport: &str) -> Result<()> {
    if transport != "tcp" && transport != "udp" {
        anyhow::bail!("unsupported transport: {transport}")
    }
    Ok(())
}

fn valid_destination(addr: &str) -> Result<()> {
    let host = addr
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("destination must be host:port"))?
        .0;
    let ip: IpAddr = host.parse()?;
    if !ip.is_loopback() {
        anyhow::bail!("destination must be loopback")
    }
    Ok(())
}
async fn read_msg(s: &mut (impl AsyncReadExt + Unpin)) -> Result<Message> {
    let mut h = [0; 4];
    s.read_exact(&mut h).await?;
    let n = u32::from_be_bytes(h) as usize;
    if n > MAX_FRAME {
        anyhow::bail!("frame too large")
    }
    let mut b = vec![0; n + 4];
    b[..4].copy_from_slice(&h);
    s.read_exact(&mut b[4..]).await?;
    Ok(decode(&b)?)
}
async fn write_msg(s: &mut (impl AsyncWriteExt + Unpin), m: &Message) -> Result<()> {
    s.write_all(&encode(m)?).await?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
async fn run(
    endpoint: String,
    token: String,
    tunnel: String,
    destination: String,
    transport: String,
    tls: bool,
    ca: Option<String>,
    server_name: Option<String>,
) -> Result<()> {
    valid_destination(&destination)?;
    let mut gateway = connect_gateway(endpoint, tls, ca, server_name).await?;
    write_msg(&mut gateway, &Message::Auth { token, tunnel }).await?;
    if read_msg(&mut gateway).await? != Message::AuthOk {
        anyhow::bail!("authentication failed")
    }
    let (mut rd, mut wr) = tokio::io::split(gateway);
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(128);
    let _writer_handle = tokio::spawn(async move {
        while let Some(m) = out_rx.recv().await {
            write_msg(&mut wr, &m).await?
        }
        Ok::<_, anyhow::Error>(())
    });
    let streams: Arc<Mutex<HashMap<u32, mpsc::Sender<Message>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let heartbeat = out_tx.clone();
    let _heartbeat_handle = tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(15)).await;
            if heartbeat.send(Message::Heartbeat).await.is_err() {
                break;
            }
        }
    });
    loop {
        match read_msg(&mut rd).await? {
            Message::Open { id } if transport == "tcp" => {
                let socket = TcpStream::connect(&destination).await;
                match socket {
                    Ok(socket) => {
                        let (mut sr, mut sw) = socket.into_split();
                        let (tx, mut rx) = mpsc::channel(64);
                        streams.lock().await.insert(id, tx);
                        let streams_writer = streams.clone();
                        tokio::spawn(async move {
                            while let Some(m) = rx.recv().await {
                                if let Message::Data { bytes, .. } = m {
                                    if sw.write_all(&bytes).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            streams_writer.lock().await.remove(&id);
                        });
                        let sender2 = out_tx.clone();
                        tokio::spawn(async move {
                            let mut b = vec![0; MAX_FRAME / 2];
                            while let Ok(n) = sr.read(&mut b).await {
                                if n == 0 {
                                    break;
                                }
                                if sender2
                                    .send(Message::Data {
                                        id,
                                        bytes: b[..n].to_vec(),
                                    })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            let _ = sender2.send(Message::Close { id }).await;
                        });
                    }
                    Err(_) => {
                        out_tx
                            .send(Message::OpenError {
                                id,
                                reason: "destination unavailable".into(),
                            })
                            .await?;
                        continue;
                    }
                }
                out_tx.send(Message::OpenOk { id }).await?;
            }
            Message::OpenUdp { id } if transport == "udp" => {
                let socket = match UdpSocket::bind("127.0.0.1:0").await {
                    Ok(socket) => socket,
                    Err(_) => {
                        out_tx
                            .send(Message::OpenError {
                                id,
                                reason: "UDP socket unavailable".into(),
                            })
                            .await?;
                        continue;
                    }
                };
                socket.connect(&destination).await?;
                let socket = Arc::new(socket);
                let (tx, mut rx) = mpsc::channel(64);
                streams.lock().await.insert(id, tx);
                let sender = out_tx.clone();
                let receive_socket = socket.clone();
                tokio::spawn(async move {
                    let mut bytes = vec![0; MAX_DATAGRAM];
                    while let Ok(n) = receive_socket.recv(&mut bytes).await {
                        if sender
                            .send(Message::Datagram {
                                id,
                                bytes: bytes[..n].to_vec(),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                tokio::spawn(async move {
                    while let Some(Message::Datagram { bytes, .. }) = rx.recv().await {
                        if bytes.len() <= MAX_DATAGRAM && socket.send(&bytes).await.is_err() {
                            break;
                        }
                    }
                });
                out_tx.send(Message::OpenOk { id }).await?;
            }
            Message::Data { id, bytes } => {
                if let Some(tx) = streams.lock().await.get(&id) {
                    tx.send(Message::Data { id, bytes }).await?
                }
            }
            Message::Close { id } => {
                streams.lock().await.remove(&id);
                out_tx.send(Message::Close { id }).await?
            }
            Message::Heartbeat => out_tx.send(Message::HeartbeatAck).await?,
            Message::HeartbeatAck => {}
            _ => {}
        }
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = env::var("SHADOW_GATEWAY").unwrap_or_else(|_| "127.0.0.1:9000".into());
    let token = env::var("SHADOW_TOKEN")?;
    let tunnel = env::var("SHADOW_TUNNEL").unwrap_or_else(|_| "default".into());
    let minecraft_type = env::var("SHADOW_MINECRAFT_TYPE").unwrap_or_else(|_| "java".into());
    valid_type(&minecraft_type)?;
    let default_transport = if minecraft_type == "bedrock" {
        "udp"
    } else {
        "tcp"
    };
    let transport = env::var("SHADOW_TRANSPORT").unwrap_or_else(|_| default_transport.into());
    valid_transport(&transport)?;
    let expected_transport = if minecraft_type == "bedrock" {
        "udp"
    } else {
        "tcp"
    };
    if transport != expected_transport {
        anyhow::bail!("transport {transport} invalid for Minecraft type {minecraft_type}")
    }
    let default_destination = if minecraft_type == "bedrock" {
        "127.0.0.1:19132"
    } else {
        "127.0.0.1:25565"
    };
    let destination = env::var("SHADOW_DESTINATION").unwrap_or_else(|_| default_destination.into());
    let tls = env::var("SHADOW_TLS").unwrap_or_default() == "1";
    let ca = env::var("SHADOW_TLS_CA").ok();
    let server_name = env::var("SHADOW_TLS_SERVER_NAME").ok();
    if tls && (ca.is_none() || server_name.is_none()) {
        anyhow::bail!("TLS enabled but SHADOW_TLS_CA/SHADOW_TLS_SERVER_NAME missing")
    }
    loop {
        if run(
            endpoint.clone(),
            token.clone(),
            tunnel.clone(),
            destination.clone(),
            transport.clone(),
            tls,
            ca.clone(),
            server_name.clone(),
        )
        .await
        .is_ok()
        {
            break;
        }
        sleep(Duration::from_secs(2)).await
    }
    Ok(())
}
