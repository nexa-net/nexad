<div align="center">

# nexad

**NexaNet daemon -- container orchestration engine**

[![CI](https://github.com/nexa-net/nexad/actions/workflows/ci.yml/badge.svg)](https://github.com/nexa-net/nexad/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

nexad is the server component of NexaNet. It provides concrete adapter
implementations for every [nexa-core](https://github.com/nexa-net/nexa-core) port
trait, exposes a REST API on port 6443, and supports single-node, master, and
worker clustering modes with gRPC transport.

</div>

---

## Features

- **Container runtimes** -- Docker (via bollard) and containerd (via ctr CLI) with automatic detection
- **Persistent state** -- SQLite-backed store for projects, deployments, pods, nodes, and cluster config
- **Encrypted secrets** -- AES-256-GCM encryption at rest with auto-generated master key
- **REST API** -- axum-based HTTP API on port 6443 with full CRUD for all resources
- **Multi-node clustering** -- master/worker topology over gRPC with join tokens and heartbeat monitoring
- **Reverse proxy integration** -- pluggable backends: nexa-proxy (built-in), nginx, caddy, traefik
- **Automatic TLS** -- ACME certificate provisioning and daily renewal
- **Overlay networking** -- WireGuard-based mesh with CNI plugin support and per-project subnet allocation
- **Embedded DNS** -- Hickory DNS server for service discovery (`<deployment>.<project>.internal`)
- **Health checking** -- background probe runner with orchestrator-driven restart
- **Container event watcher** -- real-time container lifecycle events fed back to the orchestrator
- **Runtime auto-detection** -- probes for Docker socket and containerd socket, falls back gracefully
- **156 unit tests + 6 integration tests**

## Quick Start

### Prerequisites

- Rust 1.85+
- Docker or containerd running on the host

### Build and run

```bash
# Build
cargo build --release

# Start in single-node mode (default)
./target/release/nexad

# Start with custom options
./target/release/nexad \
    --host 0.0.0.0 \
    --port 6443 \
    --data-dir /var/lib/nexa \
    --runtime auto
```

### Deploy a service

```bash
# Using the CLI
nexa deploy examples/app.yaml

# Or directly via the API
curl -X POST http://localhost:6443/api/v1/deploy \
    -H 'Content-Type: application/json' \
    -d @spec.json
```

## Deployment Specs

nexad accepts YAML deployment specs. Two examples are included:

**examples/app.yaml** -- a multi-replica API service:

```yaml
project: ecommerce

deployment:
  name: api

replicas: 3
image: ghcr.io/company/api:latest

ports:
  - 3000

env:
  DATABASE_URL: "postgres://localhost/ecommerce"
  REDIS_URL: "redis://localhost:6379"

network:
  public: true
  domain: api.example.com
  https: true

healthcheck:
  path: /health
  interval: 10s
```

**examples/nginx.yaml** -- a simple web server:

```yaml
project: demo

deployment:
  name: nginx

replicas: 1
image: nginx:alpine

ports:
  - 8080

network:
  public: true

healthcheck:
  path: /
  interval: 10s
```

## CLI Flags

```
nexad [OPTIONS]

Options:
    --host <HOST>               Listen address [default: 0.0.0.0]
    --port <PORT>               HTTP API port [default: 6443]
    --data-dir <DIR>            Data directory [default: /var/lib/nexa]
    --mode <MODE>               Node mode: single, master, worker [default: single]
    --runtime <RUNTIME>         Container runtime: docker, containerd, auto [default: auto]
    --join <ADDR>               Master address (worker mode)
    --token <TOKEN>             Join token (worker mode)
    --grpc-port <PORT>          gRPC port for cluster communication [default: 6444]
    --dns-mode <MODE>           DNS mode: noop, embedded [default: noop]
    --dns-listen <ADDR>         Embedded DNS listen address [default: 0.0.0.0:15353]
    --dns-upstream <ADDR>       Upstream DNS server [default: 8.8.8.8:53]
    --master-ip <IP>            Node IP for container DNS config (embedded mode)
    --proxy-backend <BACKEND>   Proxy: nexa-proxy, nginx, caddy, traefik [default: nexa-proxy]
    --proxy-config-dir <DIR>    Proxy config directory [default: /var/lib/nexa/proxy]
    --acme-email <EMAIL>        ACME email for automatic TLS
    --cluster-cidr <CIDR>       Overlay network CIDR [default: 172.20.0.0/16]
    --wg-port <PORT>            WireGuard listen port [default: 51820]
    --overlay                   Enable WireGuard overlay network
```

## Architecture

```
                    +-------------------+
                    |    nexa (CLI)     |
                    +--------+----------+
                             |  HTTP
                             v
+-----------------------------------------------------------+
|  nexad                                                    |
|                                                           |
|  +------------------+    +-----------------------------+  |
|  |   REST API       |    |   gRPC Cluster Server       |  |
|  |   (axum :6443)   |    |   (tonic :6444)             |  |
|  +--------+---------+    +-------------+---------------+  |
|           |                            |                  |
|           v                            v                  |
|  +--------------------------------------------------+    |
|  |              Orchestrator (actor loop)            |    |
|  |   mpsc/oneshot channels -- 30+ command variants   |    |
|  +------+-------+-------+-------+-------+-----------+    |
|         |       |       |       |       |                 |
|         v       v       v       v       v                 |
|  +---------+ +-----+ +------+ +-----+ +-------+          |
|  |Container| |State| |Secret| | DNS | | Proxy  |         |
|  |Runtime  | |Store| |Store | |     | |Backend |         |
|  +---------+ +-----+ +------+ +-----+ +-------+          |
|   Docker/    SQLite   AES-GCM  Hickory  nginx/caddy/     |
|   containerd          SQLite   DNS      traefik/nexa     |
+-----------------------------------------------------------+
```

### Adapter implementations

| Port Trait | Adapter | Details |
|---|---|---|
| `ContainerRuntime` | `DockerRuntime` | bollard crate, Docker Engine API |
| `ContainerRuntime` | `ContainerdRuntime` | ctr CLI wrapper |
| `StateStore` | `SqliteStore` | sqlx with migrations |
| `SecretStore` | `EncryptedSqliteSecretStore` | AES-256-GCM, rusqlite |
| `ClusterTransport` | gRPC client/server | tonic + prost, protobuf |
| `ClusterTransport` | `LocalTransport` | single-node passthrough |
| `DnsProvider` | `HickoryDnsProvider` | embedded DNS server |
| `DnsProvider` | `NoopDnsProvider` | Docker DNS fallback |
| `ProxyBackend` | `NexaProxyBackend` | JSON config for nexa-proxy |
| `ProxyBackend` | `NginxBackend` | generates nginx.conf |
| `ProxyBackend` | `CaddyBackend` | generates Caddyfile + API reload |
| `ProxyBackend` | `TraefikBackend` | generates dynamic YAML config |

## REST API

All endpoints are under `/api/v1/`. The API listens on port 6443 by default.

| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/health` | Health check |
| `POST` | `/api/v1/deploy` | Deploy from spec |
| `GET` | `/api/v1/deployments` | List deployments |
| `POST` | `/api/v1/projects` | Create project |
| `GET` | `/api/v1/projects` | List projects |
| `POST` | `/api/v1/projects/{name}/suspend` | Suspend project |
| `POST` | `/api/v1/projects/{name}/resume` | Resume project |
| `DELETE` | `/api/v1/projects/{name}` | Delete project |
| `POST` | `/api/v1/projects/{p}/deployments/{d}/scale` | Scale deployment |
| `POST` | `/api/v1/projects/{p}/deployments/{d}/stop` | Stop deployment |
| `DELETE` | `/api/v1/projects/{p}/deployments/{d}` | Remove deployment |
| `GET` | `/api/v1/projects/{p}/deployments/{d}/logs` | Stream logs |
| `GET` | `/api/v1/pods` | List pods |
| `GET/POST` | `/api/v1/projects/{p}/secrets[/{n}]` | Manage secrets |
| `GET` | `/api/v1/nodes` | List nodes |
| `POST` | `/api/v1/nodes/{name}/drain` | Drain node |
| `DELETE` | `/api/v1/nodes/{name}` | Remove node |
| `GET/POST` | `/api/v1/routes` | List/add routes |
| `DELETE` | `/api/v1/routes/{domain}` | Remove route |
| `POST` | `/api/v1/certs/import` | Import TLS certificate |
| `POST` | `/api/v1/cluster/init` | Initialize cluster |
| `GET` | `/api/v1/cluster/token` | Show join token |
| `POST` | `/api/v1/cluster/token/rotate` | Rotate join token |
| `GET/POST` | `/api/v1/cluster/scheduler` | Scheduler config |

## Clustering

nexad supports a master/worker topology for multi-node deployments.

```bash
# Start the master
nexad --mode master --dns-mode embedded --master-ip 10.0.1.1 --overlay

# The master prints a join command:
#   nexad --mode worker --join 10.0.1.1:6444 --token <TOKEN>

# On worker nodes
nexad --mode worker \
    --join 10.0.1.1:6444 \
    --token <TOKEN> \
    --overlay
```

Workers register via gRPC, send periodic heartbeats, and receive pod assignments
from the master. The heartbeat monitor detects dead nodes and flags pods for
rescheduling.

## Development

```bash
# Build
cargo build

# Run all tests (156 unit + 6 integration)
cargo test

# Run only unit tests
cargo test --lib

# Run integration tests (requires Docker)
cargo test --test runtime_integration
cargo test --test sqlite_integration
```

## Related Repositories

| Repository | Description |
|---|---|
| [nexa-core](https://github.com/nexa-net/nexa-core) | Core domain types, traits, and orchestrator |
| [nexa-cli](https://github.com/nexa-net/nexa-cli) | CLI tool for deploying and managing containers |
| [nexa-proxy](https://github.com/nexa-net/nexa-proxy) | Lightweight reverse proxy with weighted load balancing |

## License

Apache-2.0 -- see [LICENSE](LICENSE) for details.
