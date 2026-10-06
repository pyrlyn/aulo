CREATE TABLE bots (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at BIGINT NOT NULL
);

CREATE TABLE chats (
    id TEXT NOT NULL PRIMARY KEY,
    bot_id TEXT REFERENCES bots (id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    model TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX chats_updated_at ON chats (updated_at);
CREATE INDEX chats_bot_id ON chats (bot_id);

CREATE TABLE messages (
    id TEXT NOT NULL PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX messages_chat_id_id ON messages (chat_id, id);

CREATE TABLE tool_calls (
    id TEXT NOT NULL PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    arguments TEXT NOT NULL,
    output TEXT,
    status TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX tool_calls_message_id ON tool_calls (message_id);

CREATE TABLE usage (
    id TEXT NOT NULL PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens BIGINT NOT NULL,
    output_tokens BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX usage_chat_id ON usage (chat_id);
