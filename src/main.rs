use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use wasmtime::*;

struct AppState {
    db: Mutex<Connection>,
    engine: Engine,
}

#[tokio::main]
async fn main() {
    let db = Connection::open("blobs.db").expect("open db");
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS blobs (
            hash TEXT PRIMARY KEY,
            data BLOB NOT NULL
        )",
    )
    .expect("create table");

    let engine = Engine::default();

    let state = Arc::new(AppState {
        db: Mutex::new(db),
        engine,
    });

    let app = Router::new()
        .route("/blobs", put(put_blob))
        .route("/blobs/:hash", get(get_blob))
        .route("/execute/:hash", post(execute))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000")
        .await
        .expect("bind");
    println!("listening on :8000");
    axum::serve(listener, app).await.expect("serve");
}

/// PUT /blobs — store a blob, return its sha256 hash
async fn put_blob(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let hash = sha256_hex(&body);
    let db = state.db.lock().unwrap();
    db.execute(
        "INSERT OR IGNORE INTO blobs (hash, data) VALUES (?1, ?2)",
        rusqlite::params![hash, body.as_ref()],
    )
    .unwrap();
    (StatusCode::OK, hash)
}

/// GET /blobs/:hash — retrieve a blob
async fn get_blob(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock().unwrap();
    let result: Result<Vec<u8>, _> = db.query_row(
        "SELECT data FROM blobs WHERE hash = ?1",
        rusqlite::params![hash],
        |row| row.get(0),
    );
    match result {
        Ok(data) => Ok((
            StatusCode::OK,
            [("content-type", "application/octet-stream")],
            data,
        )),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

/// POST /execute/:hash — execute a wasm module
///
/// Wasm contract:
///   The module exports `memory` and `run(input_ptr: i32, input_len: i32) -> i32`.
///   The host writes input into the module's memory at a fixed offset.
///   `run` returns a pointer to `[output_len: u32 LE][output_bytes...]`.
async fn execute(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // Load wasm bytes from DB
    let wasm_bytes: Vec<u8> = {
        let db = state.db.lock().unwrap();
        match db.query_row(
            "SELECT data FROM blobs WHERE hash = ?1",
            rusqlite::params![hash],
            |row| row.get(0),
        ) {
            Ok(b) => b,
            Err(_) => return Err((StatusCode::NOT_FOUND, "blob not found".to_string())),
        }
    };

    // Compile & run (blocking work — spawn_blocking keeps the runtime happy)
    let input = body.to_vec();
    let engine = state.engine.clone();

    let result = tokio::task::spawn_blocking(move || run_wasm(&engine, &wasm_bytes, &input))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("join: {e}")))?;

    match result {
        Ok(output) => Ok((
            StatusCode::OK,
            [("content-type", "application/octet-stream")],
            output,
        )),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("wasm: {e}"))),
    }
}

/// Input is written at this offset in the wasm module's memory.
const INPUT_OFFSET: usize = 0x10000; // 64 KiB

fn run_wasm(engine: &Engine, wasm_bytes: &[u8], input: &[u8]) -> anyhow::Result<Vec<u8>> {
    let module = Module::new(engine, wasm_bytes)?;
    let mut store = Store::new(engine, ());
    let instance = Instance::new(&mut store, &module, &[])?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("module must export 'memory'"))?;

    // Grow memory if needed to fit input
    let needed = INPUT_OFFSET + input.len();
    let current = memory.data_size(&store);
    if needed > current {
        let pages = ((needed - current) + 65535) / 65536;
        memory.grow(&mut store, pages as u64)?;
    }

    // Write input into memory
    memory.data_mut(&mut store)[INPUT_OFFSET..INPUT_OFFSET + input.len()]
        .copy_from_slice(input);

    // Call run(input_ptr, input_len) -> output_ptr
    let run = instance.get_typed_func::<(i32, i32), i32>(&mut store, "run")?;
    let out_ptr = run.call(&mut store, (INPUT_OFFSET as i32, input.len() as i32))? as usize;

    // Read output: [len: u32 LE][data...]
    let mem = memory.data(&store);
    if out_ptr + 4 > mem.len() {
        anyhow::bail!("output pointer out of bounds");
    }
    let out_len = u32::from_le_bytes(mem[out_ptr..out_ptr + 4].try_into()?) as usize;
    if out_ptr + 4 + out_len > mem.len() {
        anyhow::bail!("output data out of bounds");
    }
    Ok(mem[out_ptr + 4..out_ptr + 4 + out_len].to_vec())
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
