CREATE TABLE xmp_output_authorities (
    root_path TEXT PRIMARY KEY,
    authority_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO schema_migrations(version) VALUES (2);
