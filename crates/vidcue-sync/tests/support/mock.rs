//! A small in-process stand-in for vidcue-server (PROTOCOL.md shapes), for offline tests.
//! It keeps the register model (later clock wins, device id breaks ties), op-id
//! idempotence, seq cursors, the key relay, shared playlists, health reports and a
//! WebSocket with hello/changed/presence/handoff/remote/ping. No TLS, no database.
#![allow(dead_code, clippy::result_large_err)]
use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use uuid::Uuid;

pub const CODE: &str = "123456";

#[derive(Clone, Debug)]
pub struct Device {
    pub user: Uuid,
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub value: Option<Value>,
    pub clock: i64,
    pub device: Uuid,
    pub seq: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Shared {
    pub title: String,
    pub owner: Uuid,
    pub members: Vec<Uuid>,
    pub entries: BTreeMap<Uuid, Record>,
    pub seq: i64,
}

#[derive(Default)]
pub struct Data {
    pub users: HashMap<String, (String, Uuid, bool)>,
    pub tokens: HashMap<String, Device>,
    pub records: BTreeMap<(Uuid, String, String), Record>,
    pub applied: HashSet<Uuid>,
    pub seq: i64,
    pub pairings: HashMap<String, (Uuid, Uuid)>,
    pub key_requests: HashMap<Uuid, (Uuid, Uuid, String, Option<String>)>,
    pub shared: BTreeMap<Uuid, Shared>,
    pub health: Vec<Value>,
    /// Status returned by the next push requests (e.g. 503), consumed one per call.
    pub fail_push: Vec<u16>,
    pub features_404: bool,
    pub jam: Option<Value>,
    pub log: Vec<String>,
}

pub struct Mock {
    pub origin: String,
    pub addr: SocketAddr,
    pub data: Arc<Mutex<Data>>,
    pub hub: broadcast::Sender<(Uuid, Value)>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
struct AppState {
    data: Arc<Mutex<Data>>,
    hub: broadcast::Sender<(Uuid, Value)>,
    origin: String,
}

fn err(status: u16, message: &str) -> Response {
    (
        StatusCode::from_u16(status).unwrap(),
        Json(json!({"error": message})),
    )
        .into_response()
}

fn device(state: &AppState, headers: &HeaderMap) -> Result<Device, Response> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| err(401, "Missing bearer token"))?;
    state
        .data
        .lock()
        .unwrap()
        .tokens
        .get(token)
        .cloned()
        .ok_or_else(|| err(401, "Invalid token"))
}

fn issue(data: &mut Data, user: Uuid, name: &str) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let token = format!("vcd_{}", Uuid::new_v4().simple());
    data.tokens.insert(
        token.clone(),
        Device {
            user,
            id,
            name: name.into(),
        },
    );
    (id, token)
}

impl Mock {
    pub async fn start() -> Self {
        let data = Arc::new(Mutex::new(Data::default()));
        let (hub, _) = broadcast::channel(256);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let origin = format!("http://127.0.0.1:{}", addr.port());
        let state = AppState {
            data: data.clone(),
            hub: hub.clone(),
            origin: origin.clone(),
        };
        let app = Router::new()
            .route("/api/login", post(login))
            .route("/api/me", get(me))
            .route("/api/devices", get(devices))
            .route("/api/devices/{id}", delete(revoke))
            .route("/api/pairing", post(pairing))
            .route("/api/pairing/claim", post(claim))
            .route("/api/sync/push", post(push))
            .route("/api/sync/pull", get(pull))
            .route("/api/features", get(features))
            .route("/api/keys/requests", get(key_list).post(key_create))
            .route("/api/keys/requests/{id}", get(key_fetch))
            .route("/api/keys/requests/{id}/grant", post(key_grant))
            .route(
                "/api/shared-playlists",
                get(shared_list).post(shared_create),
            )
            .route("/api/shared-playlists/{id}/members", post(shared_member))
            .route(
                "/api/shared-playlists/{id}/members/{user}",
                delete(shared_member_remove),
            )
            .route("/api/jam", get(jam_current).post(jam_create))
            .route("/api/jam/{id}/leave", post(jam_leave))
            .route("/api/shared-playlists/{id}/push", post(shared_push))
            .route("/api/shared-playlists/{id}/pull", get(shared_pull))
            .route("/api/health/reports", post(health))
            .route("/api/ws", get(ws))
            .with_state(state);
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            addr,
            data,
            hub,
            task,
        }
    }

    pub fn add_user(&self, username: &str, password: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.data
            .lock()
            .unwrap()
            .users
            .insert(username.into(), (password.into(), id, false));
        id
    }

    /// Writes a register directly, as another device would.
    pub fn put(
        &self,
        user: Uuid,
        collection: &str,
        key: &str,
        value: Option<Value>,
        clock: i64,
        device: Uuid,
    ) -> i64 {
        let mut data = self.data.lock().unwrap();
        data.seq += 1;
        let seq = data.seq;
        data.records.insert(
            (user, collection.into(), key.into()),
            Record {
                value,
                clock,
                device,
                seq,
            },
        );
        seq
    }

    pub fn get(&self, user: Uuid, collection: &str, key: &str) -> Option<Record> {
        self.data
            .lock()
            .unwrap()
            .records
            .get(&(user, collection.into(), key.into()))
            .cloned()
    }

    pub fn revoke_all(&self, user: Uuid) {
        self.data
            .lock()
            .unwrap()
            .tokens
            .retain(|_, d| d.user != user);
    }

    pub fn log(&self) -> Vec<String> {
        self.data.lock().unwrap().log.clone()
    }
}

async fn login(State(s): State<AppState>, Json(body): Json<Value>) -> Response {
    let mut data = s.data.lock().unwrap();
    data.log.push("POST /api/login".into());
    let username = body["username"].as_str().unwrap_or_default();
    let Some((password, user, _)) = data.users.get(username).cloned() else {
        return err(401, "Wrong username, password or authenticator code");
    };
    if body["password"] != json!(password) || body["code"] != json!(CODE) {
        return err(401, "Wrong username, password or authenticator code");
    }
    let (device, token) = issue(&mut data, user, body["device_name"].as_str().unwrap_or("x"));
    Json(json!({"user_id": user, "device_id": device, "token": token})).into_response()
}

async fn me(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let data = s.data.lock().unwrap();
    let (name, admin) = data
        .users
        .iter()
        .find(|(_, (_, id, _))| *id == d.user)
        .map(|(n, (_, _, a))| (n.clone(), *a))
        .unwrap();
    Json(json!({"user_id": d.user, "username": name, "is_admin": admin, "device_id": d.id}))
        .into_response()
}

async fn devices(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let data = s.data.lock().unwrap();
    let list: Vec<Value> = data
        .tokens
        .values()
        .filter(|x| x.user == d.user)
        .map(|x| json!({"id": x.id, "name": x.name, "created_at": 1, "last_seen": 2, "paired_by": null, "current": x.id == d.id}))
        .collect();
    Json(json!({"devices": list})).into_response()
}

async fn revoke(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    s.data
        .lock()
        .unwrap()
        .tokens
        .retain(|_, x| !(x.user == d.user && x.id == id));
    let _ = s
        .hub
        .send((d.user, json!({"type": "device_revoked", "device_id": id})));
    Json(json!({"revoked": [id]})).into_response()
}

async fn pairing(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let mut data = s.data.lock().unwrap();
    let ok = data
        .users
        .values()
        .any(|(p, id, _)| *id == d.user && body["password"] == json!(p));
    if !ok {
        return err(401, "Wrong password");
    }
    let code = "ABCDEFGH23".to_owned();
    data.pairings.insert(code.clone(), (d.user, d.id));
    let server: String = url::form_urlencoded::byte_serialize(s.origin.as_bytes()).collect();
    let uri = format!("vidcue://pair?server={server}&code={code}");
    Json(json!({"code": code, "uri": uri, "qr_svg": "<svg/>", "expires_at": 9})).into_response()
}

async fn claim(State(s): State<AppState>, Json(body): Json<Value>) -> Response {
    let mut data = s.data.lock().unwrap();
    let code = body["code"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let Some((user, _)) = data.pairings.remove(&code) else {
        return err(400, "This pairing code is invalid, used or expired");
    };
    let (device, token) = issue(&mut data, user, body["device_name"].as_str().unwrap_or("x"));
    Json(json!({"user_id": user, "device_id": device, "token": token})).into_response()
}

async fn push(State(s): State<AppState>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let mut data = s.data.lock().unwrap();
    data.log.push("POST /api/sync/push".into());
    if !data.fail_push.is_empty() {
        let status = data.fail_push.remove(0);
        return err(status, "Injected failure");
    }
    let mut results = Vec::new();
    let mut latest = None;
    for op in body["ops"].as_array().unwrap() {
        let id: Uuid = serde_json::from_value(op["id"].clone()).unwrap();
        let collection = op["collection"].as_str().unwrap().to_owned();
        let key = op["key"].as_str().unwrap().to_owned();
        let clock = op["clock"].as_i64().unwrap();
        if key == "REJECT" {
            results.push(json!({"id": id, "status": "rejected", "reason": "unknown collection"}));
            continue;
        }
        if !data.applied.insert(id) {
            results.push(json!({"id": id, "status": "duplicate"}));
            continue;
        }
        let slot = (d.user, collection, key);
        if let Some(r) = data.records.get(&slot)
            && (clock, d.id) <= (r.clock, r.device)
        {
            results.push(json!({"id": id, "status": "superseded"}));
            continue;
        }
        data.seq += 1;
        let seq = data.seq;
        data.records.insert(
            slot,
            Record {
                value: op.get("value").cloned().filter(|v| !v.is_null()),
                clock,
                device: d.id,
                seq,
            },
        );
        latest = Some(seq);
        results.push(json!({"id": id, "status": "applied", "seq": seq}));
    }
    if let Some(seq) = latest {
        let _ = s.hub.send((
            d.user,
            json!({"type": "changed", "seq": seq, "device_id": d.id}),
        ));
    }
    Json(json!({"results": results, "seq": latest})).into_response()
}

async fn pull(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let since: i64 = q.get("since").and_then(|v| v.parse().ok()).unwrap_or(0);
    let limit: usize = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(1000);
    let mut data = s.data.lock().unwrap();
    data.log.push(format!("GET /api/sync/pull?since={since}"));
    let mut changes: Vec<(i64, Value)> = data
        .records
        .iter()
        .filter(|((u, _, _), r)| *u == d.user && r.seq > since)
        .map(|((_, c, k), r)| {
            (
                r.seq,
                json!({"collection": c, "key": k, "value": r.value, "clock": r.clock, "device_id": r.device, "seq": r.seq}),
            )
        })
        .collect();
    changes.sort_by_key(|(seq, _)| *seq);
    let more = changes.len() > limit;
    changes.truncate(limit);
    let next = changes.last().map_or(since, |(seq, _)| *seq);
    let changes: Vec<Value> = changes.into_iter().map(|(_, v)| v).collect();
    Json(json!({"changes": changes, "next": next, "more": more})).into_response()
}

async fn features(State(s): State<AppState>) -> Response {
    if s.data.lock().unwrap().features_404 {
        return err(404, "Not found");
    }
    Json(json!({"features": ["health", "stats", "recs", "admin_web"], "recs_available": true}))
        .into_response()
}

async fn key_create(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let id = Uuid::new_v4();
    s.data.lock().unwrap().key_requests.insert(
        id,
        (
            d.user,
            d.id,
            body["public_key"].as_str().unwrap().into(),
            None,
        ),
    );
    let _ = s.hub.send((
        d.user,
        json!({"type": "key_request", "id": id, "device_id": d.id}),
    ));
    Json(json!({"id": id, "expires_at": i64::MAX / 2})).into_response()
}

async fn key_list(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let data = s.data.lock().unwrap();
    let list: Vec<Value> = data
        .key_requests
        .iter()
        .filter(|(_, (u, dev, _, sealed))| *u == d.user && *dev != d.id && sealed.is_none())
        .map(|(id, (_, dev, public, _))| {
            let name = data.tokens.values().find(|t| t.id == *dev).map(|t| t.name.clone()).unwrap_or_default();
            json!({"id": id, "device_id": dev, "device_name": name, "public_key": public, "created_at": 1, "expires_at": i64::MAX / 2})
        })
        .collect();
    Json(json!({"requests": list})).into_response()
}

async fn key_grant(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let mut data = s.data.lock().unwrap();
    let Some(request) = data
        .key_requests
        .get_mut(&id)
        .filter(|r| r.0 == d.user && r.1 != d.id)
    else {
        return err(404, "No open key request with this id");
    };
    request.3 = Some(body["sealed_key"].as_str().unwrap().into());
    let requester = request.1;
    let _ = s.hub.send((
        d.user,
        json!({"type": "key_granted", "id": id, "device_id": requester}),
    ));
    Json(json!({"granted": id})).into_response()
}

async fn key_fetch(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let mut data = s.data.lock().unwrap();
    let Some(request) = data.key_requests.get(&id).filter(|r| r.1 == d.id).cloned() else {
        return err(404, "No such key request");
    };
    if request.3.is_some() {
        data.key_requests.remove(&id);
    }
    Json(json!({"id": id, "sealed_key": request.3})).into_response()
}

fn username(data: &Data, user: Uuid) -> String {
    data.users
        .iter()
        .find(|(_, (_, id, _))| *id == user)
        .map(|(n, _)| n.clone())
        .unwrap_or_default()
}

fn shared_json(data: &Data, id: Uuid, p: &Shared, caller: Uuid) -> Value {
    let seq = p
        .entries
        .values()
        .map(|e| e.seq)
        .max()
        .unwrap_or(0)
        .max(p.seq);
    json!({
        "id": id, "title": p.title,
        "owner": {"user_id": p.owner, "username": username(data, p.owner)},
        "members": p.members.iter().map(|m| json!({"user_id": m, "username": username(data, *m)})).collect::<Vec<_>>(),
        "entry_count": p.entries.values().filter(|e| e.value.is_some()).count(),
        "seq": seq, "updated_at": 1,
        "role": if p.owner == caller { "owner" } else { "member" },
    })
}

async fn shared_list(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let data = s.data.lock().unwrap();
    let list: Vec<Value> = data
        .shared
        .iter()
        .filter(|(_, p)| p.members.contains(&d.user))
        .map(|(id, p)| shared_json(&data, *id, p, d.user))
        .collect();
    Json(json!({"playlists": list})).into_response()
}

async fn shared_create(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let id: Uuid = serde_json::from_value(body["id"].clone()).unwrap();
    let mut data = s.data.lock().unwrap();
    data.seq += 1;
    let seq = data.seq;
    let playlist = data.shared.entry(id).or_insert_with(|| Shared {
        title: body["title"].as_str().unwrap_or_default().into(),
        owner: d.user,
        members: vec![d.user],
        entries: BTreeMap::new(),
        seq,
    });
    let playlist = playlist.clone();
    (
        StatusCode::CREATED,
        Json(shared_json(&data, id, &playlist, d.user)),
    )
        .into_response()
}

async fn shared_member(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let user: Uuid = serde_json::from_value(body["user_id"].clone()).unwrap();
    let mut data = s.data.lock().unwrap();
    data.seq += 1;
    let seq = data.seq;
    let Some(p) = data.shared.get_mut(&id).filter(|p| p.owner == d.user) else {
        return err(404, "No such playlist");
    };
    if !p.members.contains(&user) {
        p.members.push(user);
    }
    p.seq = seq;
    let p = p.clone();
    Json(shared_json(&data, id, &p, d.user)).into_response()
}

async fn shared_push(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let mut data = s.data.lock().unwrap();
    if !data
        .shared
        .get(&id)
        .is_some_and(|p| p.members.contains(&d.user))
    {
        return err(404, "No such playlist");
    }
    let mut results = Vec::new();
    for op in body["ops"].as_array().unwrap() {
        let op_id: Uuid = serde_json::from_value(op["id"].clone()).unwrap();
        let entry: Uuid = serde_json::from_value(op["entry_id"].clone()).unwrap();
        let clock = op["clock"].as_i64().unwrap();
        data.seq += 1;
        let seq = data.seq;
        let p = data.shared.get_mut(&id).unwrap();
        if p.entries
            .get(&entry)
            .is_some_and(|r| (clock, d.id) <= (r.clock, r.device))
        {
            results.push(json!({"id": op_id, "status": "superseded"}));
            continue;
        }
        p.entries.insert(
            entry,
            Record {
                value: op.get("value").cloned().filter(|v| !v.is_null()),
                clock,
                device: d.id,
                seq,
            },
        );
        results.push(json!({"id": op_id, "status": "applied", "seq": seq}));
    }
    let members = data.shared[&id].members.clone();
    for m in members {
        let _ = s.hub.send((m, json!({"type": "shared_playlist_changed", "playlist_id": id, "seq": data.seq, "device_id": d.id})));
    }
    Json(json!({"results": results, "seq": data.seq})).into_response()
}

async fn shared_pull(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let since: i64 = q.get("since").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut data = s.data.lock().unwrap();
    data.log
        .push(format!("GET /api/shared-playlists/{id}/pull?since={since}"));
    let Some(p) = data
        .shared
        .get(&id)
        .filter(|p| p.members.contains(&d.user))
        .cloned()
    else {
        return err(404, "No such playlist");
    };
    let mut changes: Vec<(i64, Value)> = p
        .entries
        .iter()
        .filter(|(_, r)| r.seq > since)
        .map(|(e, r)| (r.seq, json!({"entry_id": e, "value": r.value, "clock": r.clock, "device_id": r.device, "user_id": d.user, "seq": r.seq})))
        .collect();
    changes.sort_by_key(|(seq, _)| *seq);
    let playlist = shared_json(&data, id, &p, d.user);
    let next = playlist["seq"].as_i64().unwrap().max(since);
    let changes: Vec<Value> = changes.into_iter().map(|(_, v)| v).collect();
    Json(json!({"playlist": playlist, "changes": changes, "next": next, "more": false}))
        .into_response()
}

async fn health(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if let Err(e) = device(&s, &headers) {
        return e;
    }
    let reports = body["reports"].as_array().cloned().unwrap_or_default();
    let n = reports.len();
    s.data.lock().unwrap().health.extend(reports);
    Json(json!({"accepted": n, "rejected": 0})).into_response()
}

async fn ws(State(s): State<AppState>, headers: HeaderMap, upgrade: WebSocketUpgrade) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    upgrade.on_upgrade(move |socket| session(s, d, socket))
}

async fn session(s: AppState, d: Device, mut socket: WebSocket) {
    let mut rx = s.hub.subscribe();
    let seq = s.data.lock().unwrap().seq;
    let hello = json!({"type": "hello", "seq": seq, "device_id": d.id, "server_ms": vidcue_sync::now_ms(), "features": ["realtime", "social", "jam"]});
    if socket
        .send(Message::Text(hello.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    loop {
        tokio::select! {
            event = rx.recv() => {
                let Ok((user, event)) = event else { return };
                if user != d.user { continue; }
                if event["type"] == "close_all" { let _ = socket.send(Message::Close(None)).await; return; }
                if let Some(to) = event["to_device"].as_str() && to != d.id.to_string() { continue; }
                if matches!(event["type"].as_str(), Some("presence" | "presence_gone")) && event["device_id"] == json!(d.id) { continue; }
                let revoked = event["type"] == "device_revoked" && event["device_id"] == json!(d.id);
                if socket.send(Message::Text(event.to_string().into())).await.is_err() { return; }
                if revoked { let _ = socket.send(Message::Close(None)).await; return; }
            }
            incoming = socket.recv() => {
                let Some(Ok(Message::Text(text))) = incoming else { return };
                let msg: Value = serde_json::from_str(text.as_str()).unwrap_or_default();
                let rid = msg.get("rid").cloned();
                let mut reply = match msg["type"].as_str() {
                    Some("ping") => Some(json!({"type": "pong", "t0": msg["t0"], "server_ms": vidcue_sync::now_ms()})),
                    Some("presence") => {
                        let _ = s.hub.send((d.user, json!({"type": "presence", "device_id": d.id, "device_name": d.name, "state": msg["state"], "server_at": 1})));
                        None
                    }
                    Some("presence_query") => Some(json!({"type": "presence_snapshot", "devices": []})),
                    Some("handoff" | "remote") => {
                        let mut event = msg.clone();
                        event.as_object_mut().unwrap().remove("rid");
                        event["from_device"] = json!(d.id);
                        event["from_name"] = json!(d.name);
                        let _ = s.hub.send((d.user, event));
                        None
                    }
                    _ => None,
                };
                if let Some(rid) = rid {
                    let r = reply.get_or_insert_with(|| json!({"type": "ack"}));
                    r["rid"] = rid;
                }
                if let Some(reply) = reply && socket.send(Message::Text(reply.to_string().into())).await.is_err() { return; }
            }
        }
    }
}

async fn shared_member_remove(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path((id, user)): Path<(Uuid, Uuid)>,
) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let mut data = s.data.lock().unwrap();
    let Some(p) = data
        .shared
        .get_mut(&id)
        .filter(|p| p.owner == d.user || user == d.user)
    else {
        return err(404, "No such playlist");
    };
    p.members.retain(|m| *m != user);
    Json(json!({"removed": true})).into_response()
}

/// The Jam room the mock starts: one current item playing from 10 s at server time now.
pub const JAM_MEDIA: &str = "https://www.youtube.com/watch?v=jjjjjjjjjjj";

async fn jam_create(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let d = match device(&s, &headers) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let now = vidcue_sync::now_ms();
    let media = json!({"id": "jjjjjjjjjjj", "title": "Jam song", "creator": "C", "url": JAM_MEDIA, "thumbnail": "",
        "thumbnails": [], "section": "", "duration": 300, "description": "", "kind": "song", "browse_id": null,
        "tracks_url": null, "live": false, "chapters": []});
    let room = json!({
        "id": Uuid::new_v4(), "title": "Friday", "host": {"user_id": d.user, "username": "alice"},
        "guests_can_control": false, "version": 1, "created_at": now, "role": "host",
        "members": [{"user_id": d.user, "username": "alice", "role": "host", "online": true}],
        "invited": [], "queue": [],
        "playback": {"item": {"item_id": Uuid::new_v4(), "media": media, "media_key": "youtube:jjjjjjjjjjj", "added_by": d.user},
                     "state": "playing", "position_ms": 10_000, "at_server_ms": now, "rate": 1.0, "version": 1},
        "server_ms": now,
    });
    s.data.lock().unwrap().jam = Some(room.clone());
    (StatusCode::CREATED, Json(room)).into_response()
}

async fn jam_current(State(s): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(e) = device(&s, &headers) {
        return e;
    }
    let room = s.data.lock().unwrap().jam.clone();
    Json(json!({"current": room, "invites": []})).into_response()
}

async fn jam_leave(State(s): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(e) = device(&s, &headers) {
        return e;
    }
    s.data.lock().unwrap().jam = None;
    Json(json!({"left": true})).into_response()
}
