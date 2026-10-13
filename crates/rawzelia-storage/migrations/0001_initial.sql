CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE locations (
    location_id TEXT PRIMARY KEY,
    root_path TEXT NOT NULL,
    availability TEXT NOT NULL,
    last_observed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE assets (
    asset_id TEXT PRIMARY KEY,
    location_id TEXT NOT NULL REFERENCES locations(location_id),
    original_path TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    captured_at TEXT,
    fingerprint_size INTEGER NOT NULL,
    fingerprint_sha256 BLOB NOT NULL,
    availability TEXT NOT NULL,
    rating TEXT NOT NULL,
    flag TEXT NOT NULL,
    label TEXT NOT NULL,
    asset_json TEXT NOT NULL
);

CREATE TABLE metadata (
    asset_id TEXT PRIMARY KEY REFERENCES assets(asset_id) ON DELETE CASCADE,
    normalized_json TEXT NOT NULL
);

CREATE TABLE collections (
    collection_id TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE collection_members (
    collection_id TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets(asset_id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, asset_id)
);

CREATE TABLE keywords (
    keyword_id INTEGER PRIMARY KEY AUTOINCREMENT,
    value TEXT NOT NULL UNIQUE
);

CREATE TABLE asset_keywords (
    asset_id TEXT NOT NULL REFERENCES assets(asset_id) ON DELETE CASCADE,
    keyword_id INTEGER NOT NULL REFERENCES keywords(keyword_id) ON DELETE CASCADE,
    PRIMARY KEY (asset_id, keyword_id)
);

CREATE TABLE revisions (
    revision_id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id TEXT NOT NULL REFERENCES assets(asset_id) ON DELETE CASCADE,
    document_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER revisions_are_immutable
BEFORE UPDATE ON revisions
BEGIN
    SELECT RAISE(ABORT, 'revisions are immutable');
END;

CREATE TABLE asset_revision_heads (
    asset_id TEXT PRIMARY KEY REFERENCES assets(asset_id) ON DELETE CASCADE,
    revision_id INTEGER NOT NULL REFERENCES revisions(revision_id)
);

CREATE TABLE jobs (
    job_id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    heartbeat_at TEXT,
    completed INTEGER NOT NULL DEFAULT 0,
    total INTEGER NOT NULL DEFAULT 0,
    error_code TEXT
);

CREATE TABLE exports (
    export_id TEXT PRIMARY KEY,
    asset_id TEXT NOT NULL REFERENCES assets(asset_id),
    parameters_json TEXT NOT NULL,
    destination TEXT NOT NULL,
    result_sha256 BLOB,
    state TEXT NOT NULL
);

CREATE TABLE cache_index (
    cache_key TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    version TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX assets_location_idx ON assets(location_id);
CREATE INDEX assets_relative_path_idx ON assets(relative_path);
CREATE INDEX assets_captured_at_idx ON assets(captured_at);
CREATE INDEX assets_rating_flag_label_idx ON assets(rating, flag, label);
CREATE INDEX revisions_asset_idx ON revisions(asset_id, revision_id);
CREATE INDEX collection_members_asset_idx ON collection_members(asset_id);
CREATE INDEX asset_keywords_keyword_idx ON asset_keywords(keyword_id, asset_id);

INSERT INTO schema_migrations(version) VALUES (1);
