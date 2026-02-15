# wasmexec

Minimal HTTP server for storing and executing WebAssembly blobs.

Built with Rust, axum, wasmtime, and rusqlite.

## API

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/blobs` | Upload a blob. Returns its SHA-256 hash. |
| `GET` | `/blobs/:hash` | Retrieve a blob by hash. |
| `POST` | `/execute/:hash` | Execute a wasm blob. Request body = input, response body = output. |

Blobs are content-addressable and immutable.

## Wasm Contract

Modules must export:
- `memory` — the module's linear memory
- `run(input_ptr: i32, input_len: i32) -> i32` — entry point

The host writes input bytes into the module's memory at offset `0x10000`.
The host then calls `run(0x10000, input_len)`.

`run` returns a pointer to the output, formatted as:
```
[output_len: u32 LE][output_bytes...]
```

No WASI. No imported functions. Pure computation.

## Run

```
cargo run
# listening on :8000
```

## Example

```bash
# Upload a wasm module
HASH=$(curl -s -X PUT --data-binary @test/echo.wasm http://localhost:8000/blobs)

# Execute it
curl -s -X POST -d "hello" http://localhost:8000/execute/$HASH
# => hello
```

## Test Modules

- `test/echo.wat` — echoes input back
- `test/reverse.wat` — reverses input bytes
- `test/hello.wat` — returns a fixed string "hi"

Compile with: `wat2wasm test/echo.wat -o test/echo.wasm`
