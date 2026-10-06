// Hand-maintained mirror of `migrations/`; the CRUD tests fail if they drift apart.

diesel::table! {
    bots (id) {
        id -> Text,
        name -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    chats (id) {
        id -> Text,
        bot_id -> Nullable<Text>,
        title -> Text,
        model -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    messages (id) {
        id -> Text,
        chat_id -> Text,
        role -> Text,
        content -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    tool_calls (id) {
        id -> Text,
        message_id -> Text,
        name -> Text,
        arguments -> Text,
        output -> Nullable<Text>,
        status -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    usage (id) {
        id -> Text,
        chat_id -> Text,
        provider -> Text,
        model -> Text,
        input_tokens -> BigInt,
        output_tokens -> BigInt,
        created_at -> BigInt,
    }
}

diesel::joinable!(chats -> bots (bot_id));
diesel::joinable!(messages -> chats (chat_id));
diesel::joinable!(tool_calls -> messages (message_id));
diesel::joinable!(usage -> chats (chat_id));

diesel::allow_tables_to_appear_in_same_query!(bots, chats, messages, tool_calls, usage);
