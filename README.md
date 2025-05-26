# simple-http-echo-server

A minimal, production-ready HTTP echo server built in Rust using [Axum](https://github.com/tokio-rs/axum). It accepts any HTTP method and returns a structured JSON response containing request metadata including headers, query parameters, request body, and timestamps.

Ideal for testing, debugging, inspecting webhooks, and serving as a learning resource.

---

## 🚀 Features

- Accepts **any** HTTP method and path
- Echoes:
  - Request method and path
  - Query parameters
  - Headers
  - Parsed JSON or raw string body
  - Request timestamps (RFC3339 and UNIX)
- Logs each request in **nginx-style** format
- Handles **graceful shutdown** (Ctrl+C or SIGTERM)
- Supports CLI and environment configuration
- Configurable **request body size limit**

---

## 🔧 Usage

```bash
cargo run --release -- [OPTIONS]
````

### Command-Line Options

| Flag              | Env Var         | Default       | Description                             |
| ----------------- | --------------- | ------------- | --------------------------------------- |
| `--bind`          | `BIND`          | `0.0.0.0`     | Listening IP address                    |
| `--port`, `-p`    | `PORT`          | `3000`        | Listening port                          |
| `--listen-addr`   | `LISTEN_ADDR`   | -             | Combined address override (`host:port`) |
| `--max-body-size` | `MAX_BODY_SIZE` | `65536`       | Max request body size in bytes          |
| `--tag`           | `SERVER_TAG`    | `echo-server` | Custom label returned in responses      |

---

## 📦 Example Output

```bash
curl -X POST http://localhost:3000/test \
  -H "Content-Type: application/json" \
  -d '{"hello": "world"}' | jq
```

```json
{
  "method": "POST",
  "path": "/test",
  "headers": {
    "content-type": "application/json",
    "user-agent": "curl/7.87.0"
  },
  "query": {},
  "body": {
    "hello": "world"
  },
  "server_tag": "echo-server",
  "server_version": "0.1.0",
  "timestamp": "2025-05-26T15:00:00Z",
  "timestamp_unix": 1748252400
}
```

---

## 🛡 Graceful Shutdown

* Press `Ctrl+C` to terminate the server
* Handles `SIGTERM` on Unix systems (e.g., in Docker or Kubernetes)

---

## 🐳 Docker


```bash
docker pull ghcr.io/rbehzadan/simple-http-echo-server:latest
docker run --rm -p 3000:3000 ghcr.io/rbehzadan/simple-http-echo-server:latest
```

---

## 📜 License

MIT License © 2025 Reza Behzadan

---

## 🙌 Acknowledgments

* Built with [Axum](https://github.com/tokio-rs/axum)
* Logging via [tracing](https://github.com/tokio-rs/tracing)

