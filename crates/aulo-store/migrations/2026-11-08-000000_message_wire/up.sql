-- The provider-form message (`llm_wire::Message` JSON). `content` keeps only the
-- text a person reads; tool use and tool results need this to resume a chat.
ALTER TABLE messages ADD COLUMN wire TEXT;
