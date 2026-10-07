-- Diesel cannot model FTS5 virtual tables, so this file is raw SQL on purpose.
--
-- External-content index: the text stays in `messages`, the index holds only
-- terms. It is keyed by `messages.rowid`; `messages` has a TEXT primary key, so
-- its rowid is implicit and a VACUUM may renumber it. The store never vacuums;
-- if that changes, rebuild with `INSERT INTO message_search(message_search) VALUES ('rebuild')`.
-- `unicode61` (the default tokenizer, set explicitly) folds case and diacritics
-- without English stemming, which would hurt a multilingual voice assistant.
CREATE VIRTUAL TABLE message_search USING fts5 (
    content,
    content = 'messages',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Messages that exist before this migration.
INSERT INTO message_search (message_search) VALUES ('rebuild');

-- An external-content table must be told what to forget, with the old text.
CREATE TRIGGER message_search_insert AFTER INSERT ON messages BEGIN
    INSERT INTO message_search (rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TRIGGER message_search_delete AFTER DELETE ON messages BEGIN
    INSERT INTO message_search (message_search, rowid, content)
    VALUES ('delete', old.rowid, old.content);
END;

CREATE TRIGGER message_search_update AFTER UPDATE OF content ON messages BEGIN
    INSERT INTO message_search (message_search, rowid, content)
    VALUES ('delete', old.rowid, old.content);
    INSERT INTO message_search (rowid, content) VALUES (new.rowid, new.content);
END;
