-- Bearer tokens for remote clients (spec §11). Only the SHA-256 of a token is kept, so a copy of
-- the database grants no access. src/commonMain/sqldelight declares the same schema, and
-- MigrationsTest fails if the two drift apart.
CREATE TABLE api_tokens (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    scope TEXT NOT NULL,
    -- Lowercase hex of the token's SHA-256.
    hash TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);
