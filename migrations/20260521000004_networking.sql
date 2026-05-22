CREATE TABLE IF NOT EXISTS routes (
    domain      TEXT PRIMARY KEY,
    project     TEXT NOT NULL,
    deployment  TEXT NOT NULL,
    tls_mode    TEXT NOT NULL DEFAULT 'none',
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS certificates (
    domain      TEXT PRIMARY KEY,
    cert_pem    BLOB NOT NULL,
    key_pem_enc BLOB NOT NULL,
    key_nonce   BLOB NOT NULL,
    issued_at   TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    acme_account TEXT
);

CREATE TABLE IF NOT EXISTS subnet_allocations (
    node_id     TEXT NOT NULL REFERENCES nodes(id),
    project     TEXT NOT NULL,
    subnet      TEXT NOT NULL UNIQUE,
    PRIMARY KEY (node_id, project)
);

CREATE INDEX IF NOT EXISTS idx_routes_project ON routes(project);
CREATE INDEX IF NOT EXISTS idx_certificates_expires ON certificates(expires_at);
CREATE INDEX IF NOT EXISTS idx_subnet_allocations_subnet ON subnet_allocations(subnet);
