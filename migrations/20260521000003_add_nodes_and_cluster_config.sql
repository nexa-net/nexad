ALTER TABLE pods ADD COLUMN node_id TEXT;

CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    address TEXT NOT NULL,
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    cpu_cores REAL NOT NULL DEFAULT 0,
    memory_bytes INTEGER NOT NULL DEFAULT 0,
    cpu_available REAL NOT NULL DEFAULT 0,
    memory_available INTEGER NOT NULL DEFAULT 0,
    running_pods INTEGER NOT NULL DEFAULT 0,
    last_heartbeat TEXT NOT NULL,
    joined_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cluster_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
