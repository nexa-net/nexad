CREATE TABLE IF NOT EXISTS projects (
    name        TEXT PRIMARY KEY,
    status      TEXT NOT NULL DEFAULT 'active',
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS deployments (
    id          TEXT PRIMARY KEY,
    project     TEXT NOT NULL REFERENCES projects(name),
    name        TEXT NOT NULL,
    spec_json   TEXT NOT NULL,
    status      TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE(project, name)
);

CREATE TABLE IF NOT EXISTS pods (
    id              TEXT PRIMARY KEY,
    deployment_id   TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    project         TEXT NOT NULL,
    deployment_name TEXT NOT NULL,
    replica_index   INTEGER NOT NULL,
    container_id    TEXT,
    status          TEXT NOT NULL,
    image           TEXT NOT NULL,
    restart_count   INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS secrets (
    project     TEXT NOT NULL,
    name        TEXT NOT NULL,
    value_enc   BLOB NOT NULL,
    nonce       BLOB NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (project, name)
);
