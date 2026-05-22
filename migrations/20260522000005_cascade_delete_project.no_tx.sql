-- Recreate deployments table with ON DELETE CASCADE on the project FK
-- so that deleting a project automatically removes its deployments (and pods cascade from deployments).
PRAGMA foreign_keys=OFF;

CREATE TABLE deployments_new (
    id          TEXT PRIMARY KEY,
    project     TEXT NOT NULL REFERENCES projects(name) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    spec_json   TEXT NOT NULL,
    status      TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE(project, name)
);

INSERT INTO deployments_new SELECT id, project, name, spec_json, status, created_at, updated_at FROM deployments;

DROP TABLE deployments;

ALTER TABLE deployments_new RENAME TO deployments;

PRAGMA foreign_keys=ON;
