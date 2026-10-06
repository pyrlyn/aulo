CREATE TABLE grants (
    id TEXT NOT NULL PRIMARY KEY,
    subject TEXT NOT NULL,
    scope TEXT NOT NULL,
    decision TEXT NOT NULL,
    -- NULL means the grant never expires.
    expires_at BIGINT,
    created_at BIGINT NOT NULL
);

CREATE INDEX grants_subject_scope ON grants (subject, scope);

-- AUTOINCREMENT so a seq is never reused, even if a row were ever removed by hand.
CREATE TABLE audit (
    seq INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    -- UNIQUE makes a second writer that read the same chain head fail instead of forking the chain.
    prev_hash TEXT NOT NULL UNIQUE,
    hash TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
