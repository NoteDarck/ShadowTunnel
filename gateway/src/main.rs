use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use shadowtunnel_protocol::{decode, encode, Message, MAX_DATAGRAM, MAX_FRAME};
use std::{
    collections::HashMap,
    env,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{mpsc, RwLock},
};

#[derive(Clone)]
struct Config {
    bind: String,
    udp_bind: String,
    agent_bind: String,
    token: String,
    tls: bool,
    cert: Option<String>,
    key: Option<String>,
    tunnel: String,
    api_bind: String,
    api_token: Option<String>,
}
impl Config {
    fn from_env() -> Result<Self> {
        let tls = env::var("SHADOW_TLS").unwrap_or_default() == "1";
        let cert = env::var("SHADOW_TLS_CERT").ok();
        let key = env::var("SHADOW_TLS_KEY").ok();
        if tls && (cert.is_none() || key.is_none()) {
            anyhow::bail!("TLS enabled but SHADOW_TLS_CERT/SHADOW_TLS_KEY missing")
        }
        Ok(Self {
            bind: env::var("SHADOW_PUBLIC_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            udp_bind: env::var("SHADOW_PUBLIC_UDP_BIND").unwrap_or_else(|_| "0.0.0.0:19132".into()),
            agent_bind: env::var("SHADOW_AGENT_BIND").unwrap_or_else(|_| "0.0.0.0:9000".into()),
            token: env::var("SHADOW_TOKEN")?,
            tls,
            cert,
            key,
            tunnel: env::var("SHADOW_TUNNEL").unwrap_or_else(|_| "default".into()),
            api_bind: env::var("SHADOW_API_BIND").unwrap_or_else(|_| "127.0.0.1:9100".into()),
            api_token: env::var("SHADOW_API_TOKEN").ok(),
        })
    }
}
type Pending = Arc<RwLock<HashMap<(String, u32), mpsc::Sender<Message>>>>;
struct Agent {
    tx: mpsc::Sender<Message>,
}
type Registry = Arc<RwLock<HashMap<String, Agent>>>;

#[derive(Clone, Default)]
struct TunnelMeta {
    requested: bool,
}
type Tunnels = Arc<RwLock<HashMap<String, TunnelMeta>>>;
#[derive(Clone)]
struct ApiState {
    cfg: Config,
    registry: Registry,
    tunnels: Tunnels,
}
#[derive(Deserialize)]
struct CreateTunnel {
    tunnel: String,
    token: Option<String>,
}
#[derive(Serialize)]
struct TunnelView {
    tunnel: String,
    online: bool,
    requested: bool,
    tcp_address: Option<String>,
    udp_address: Option<String>,
}
#[derive(Serialize)]
struct Health {
    status: &'static str,
}

fn authorized(headers: &HeaderMap, cfg: &Config) -> bool {
    match &cfg.api_token {
        Some(token) => {
            headers
                .get("SHADOW_API_TOKEN")
                .and_then(|v| v.to_str().ok())
                .or_else(|| {
                    headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.strip_prefix("Bearer "))
                })
                == Some(token)
        }
        None => false,
    }
}
fn unauthorized() -> impl IntoResponse {
    (StatusCode::UNAUTHORIZED, "unauthorized")
}
async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}
async fn tunnels(State(state): State<ApiState>, headers: HeaderMap) -> impl IntoResponse {
    if !authorized(&headers, &state.cfg) {
        return (StatusCode::UNAUTHORIZED, Json(Vec::<TunnelView>::new())).into_response();
    }
    let metas = state.tunnels.read().await;
    let registry = state.registry.read().await;
    let views = metas
        .iter()
        .map(|(name, meta)| TunnelView {
            tunnel: name.clone(),
            online: registry.contains_key(name),
            requested: meta.requested,
            tcp_address: registry.contains_key(name).then(|| state.cfg.bind.clone()),
            udp_address: registry
                .contains_key(name)
                .then(|| state.cfg.udp_bind.clone()),
        })
        .collect::<Vec<_>>();
    Json(views).into_response()
}
async fn create_tunnel(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreateTunnel>,
) -> impl IntoResponse {
    if !authorized(&headers, &state.cfg) {
        return unauthorized().into_response();
    }
    if body
        .token
        .as_deref()
        .is_some_and(|token| token != state.cfg.token)
    {
        return unauthorized().into_response();
    }
    if body.tunnel.is_empty() || body.tunnel.len() > 128 || body.tunnel.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid tunnel").into_response();
    }
    state
        .tunnels
        .write()
        .await
        .entry(body.tunnel.clone())
        .or_default();
    let online = state.registry.read().await.contains_key(&body.tunnel);
    Json(TunnelView {
        tunnel: body.tunnel,
        online,
        requested: false,
        tcp_address: online.then(|| state.cfg.bind.clone()),
        udp_address: online.then(|| state.cfg.udp_bind.clone()),
    })
    .into_response()
}
async fn set_tunnel(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(tunnel): Path<String>,
    requested: bool,
) -> impl IntoResponse {
    if !authorized(&headers, &state.cfg) {
        return unauthorized().into_response();
    }
    let mut tunnels = state.tunnels.write().await;
    let Some(meta) = tunnels.get_mut(&tunnel) else {
        return (StatusCode::NOT_FOUND, "tunnel not found").into_response();
    };
    meta.requested = requested;
    drop(tunnels);
    status(State(state), headers, Path(tunnel))
        .await
        .into_response()
}
async fn connect(
    state: State<ApiState>,
    headers: HeaderMap,
    path: Path<String>,
) -> impl IntoResponse {
    set_tunnel(state, headers, path, true).await
}
async fn disconnect(
    state: State<ApiState>,
    headers: HeaderMap,
    path: Path<String>,
) -> impl IntoResponse {
    set_tunnel(state, headers, path, false).await
}
async fn status(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(tunnel): Path<String>,
) -> impl IntoResponse {
    if !authorized(&headers, &state.cfg) {
        return unauthorized().into_response();
    }
    let meta = state.tunnels.read().await.get(&tunnel).cloned();
    let Some(meta) = meta else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"tunnel not found"})),
        )
            .into_response();
    };
    let online = state.registry.read().await.contains_key(&tunnel);
    Json(TunnelView {
        tunnel,
        online,
        requested: meta.requested,
        tcp_address: online.then(|| state.cfg.bind.clone()),
        udp_address: online.then(|| state.cfg.udp_bind.clone()),
    })
    .into_response()
}
async fn read_msg(s: &mut TcpStream) -> Result<Message> {
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
async fn write_msg(s: &mut TcpStream, m: &Message) -> Result<()> {
    s.write_all(&encode(m)?).await?;
    Ok(())
}
async fn agent(
    mut stream: TcpStream,
    cfg: Config,
    registry: Registry,
    pending: Pending,
) -> Result<()> {
    let Message::Auth { token, tunnel } = read_msg(&mut stream).await? else {
        anyhow::bail!("authentication required")
    };
    if token != cfg.token {
        write_msg(&mut stream, &Message::AuthError).await?;
        anyhow::bail!("invalid token")
    }
    write_msg(&mut stream, &Message::AuthOk).await?;
    let (tx, mut rx) = mpsc::channel(128);
    registry.write().await.insert(tunnel.clone(), Agent { tx });
    loop {
        tokio::select! {r=read_msg(&mut stream)=>match r?{Message::Heartbeat=>write_msg(&mut stream,&Message::HeartbeatAck).await?,m=>{if let Some(ch)=pending.read().await.get(&(tunnel.clone(),msg_id(&m))).cloned(){let _=ch.send(m).await;}}},Some(m)=rx.recv()=>write_msg(&mut stream,&m).await?,else=>break}
    }
    registry.write().await.remove(&tunnel);
    pending.write().await.retain(|(t, _), _| t != &tunnel);
    Ok(())
}
fn msg_id(m: &Message) -> u32 {
    match m {
        Message::Open { id }
        | Message::OpenOk { id }
        | Message::OpenError { id, .. }
        | Message::Data { id, .. }
        | Message::Close { id }
        | Message::OpenUdp { id }
        | Message::Datagram { id, .. }
        | Message::CloseUdp { id } => *id,
        _ => 0,
    }
}
async fn public(
    stream: TcpStream,
    registry: Registry,
    pending: Pending,
    ids: Arc<AtomicU32>,
    tunnel: String,
) -> Result<()> {
    let agent = registry
        .read()
        .await
        .get(&tunnel)
        .map(|a| a.tx.clone())
        .ok_or_else(|| anyhow::anyhow!("configured tunnel unavailable"))?;
    let id = ids.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = mpsc::channel(64);
    pending.write().await.insert((tunnel.clone(), id), tx);
    agent.send(Message::Open { id }).await?;
    let opened = rx
        .recv()
        .await
        .ok_or_else(|| anyhow::anyhow!("agent disconnected"))?;
    if !matches!(opened, Message::OpenOk { .. }) {
        pending.write().await.remove(&(tunnel, id));
        anyhow::bail!("agent refused connection")
    }
    let (mut rd, mut wr) = stream.into_split();
    let a = agent.clone();
    let up = tokio::spawn(async move {
        let mut b = vec![0; MAX_FRAME / 2];
        while let Ok(n) = rd.read(&mut b).await {
            if n == 0 {
                break;
            }
            if a.send(Message::Data {
                id,
                bytes: b[..n].to_vec(),
            })
            .await
            .is_err()
            {
                break;
            }
        }
        let _ = a.send(Message::Close { id }).await;
    });
    while let Some(m) = rx.recv().await {
        match m {
            Message::Data { bytes, .. } => wr.write_all(&bytes).await?,
            Message::Close { .. } | Message::OpenError { .. } => break,
            _ => {}
        }
    }
    pending.write().await.remove(&(tunnel, id));
    up.abort();
    Ok(())
}
async fn public_udp(
    socket: Arc<UdpSocket>,
    registry: Registry,
    pending: Pending,
    ids: Arc<AtomicU32>,
    tunnel: String,
) -> Result<()> {
    let mut clients = HashMap::new();
    let mut buffer = vec![0; MAX_DATAGRAM];
    loop {
        let (n, address) = socket.recv_from(&mut buffer).await?;
        if n > MAX_DATAGRAM {
            continue;
        }
        let id = if let Some(id) = clients.get(&address) {
            *id
        } else {
            if clients.len() >= 128 {
                continue;
            }
            let agent = match registry.read().await.get(&tunnel).map(|a| a.tx.clone()) {
                Some(agent) => agent,
                None => continue,
            };
            let id = ids.fetch_add(1, Ordering::Relaxed);
            let (tx, mut rx) = mpsc::channel(64);
            pending.write().await.insert((tunnel.clone(), id), tx);
            if agent.send(Message::OpenUdp { id }).await.is_err() {
                pending.write().await.remove(&(tunnel.clone(), id));
                continue;
            }
            let opened = matches!(rx.recv().await, Some(Message::OpenOk { .. }));
            if !opened {
                pending.write().await.remove(&(tunnel.clone(), id));
                continue;
            }
            let reply_socket = socket.clone();
            tokio::spawn(async move {
                while let Some(message) = rx.recv().await {
                    match message {
                        Message::Datagram { bytes, .. } if bytes.len() <= MAX_DATAGRAM => {
                            if reply_socket.send_to(&bytes, address).await.is_err() {
                                break;
                            }
                        }
                        Message::CloseUdp { .. } | Message::OpenError { .. } => break,
                        _ => {}
                    }
                }
            });
            clients.insert(address, id);
            id
        };
        let agent = match registry.read().await.get(&tunnel).map(|a| a.tx.clone()) {
            Some(agent) => agent,
            None => return Ok(()),
        };
        if agent
            .send(Message::Datagram {
                id,
                bytes: buffer[..n].to_vec(),
            })
            .await
            .is_err()
        {
            clients.remove(&address);
            pending.write().await.remove(&(tunnel.clone(), id));
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::from_env()?;
    let _tls = (cfg.tls, cfg.cert.clone(), cfg.key.clone());
    let registry = Arc::new(RwLock::new(HashMap::new()));
    let tunnel_state = Arc::new(RwLock::new(HashMap::from([(
        cfg.tunnel.clone(),
        TunnelMeta::default(),
    )])));
    let api_state = ApiState {
        cfg: cfg.clone(),
        registry: registry.clone(),
        tunnels: tunnel_state.clone(),
    };
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/tunnels", get(tunnels).post(create_tunnel))
        .route("/api/tunnels/:tunnel/connect", post(connect))
        .route("/api/tunnels/:tunnel/disconnect", post(disconnect))
        .route("/api/tunnels/:tunnel/status", get(status))
        .with_state(api_state);
    let api_listener = tokio::net::TcpListener::bind(&cfg.api_bind).await?;
    tokio::spawn(async move {
        let _ = axum::serve(api_listener, api).await;
    });
    let pending = Arc::new(RwLock::new(HashMap::new()));
    let ids = Arc::new(AtomicU32::new(1));
    let agents = TcpListener::bind(&cfg.agent_bind).await?;
    let public_listener = TcpListener::bind(&cfg.bind).await?;
    let udp_listener = Arc::new(UdpSocket::bind(&cfg.udp_bind).await?);
    let r = registry.clone();
    let p = pending.clone();
    let c = cfg.clone();
    tokio::spawn(async move {
        loop {
            if let Ok((s, _)) = agents.accept().await {
                let _ = agent(s, c.clone(), r.clone(), p.clone()).await;
            }
        }
    });
    tokio::spawn(public_udp(
        udp_listener,
        registry.clone(),
        pending.clone(),
        ids.clone(),
        cfg.tunnel.clone(),
    ));
    loop {
        let (s, _) = public_listener.accept().await?;
        tokio::spawn(public(
            s,
            registry.clone(),
            pending.clone(),
            ids.clone(),
            cfg.tunnel.clone(),
        ));
    }
}
