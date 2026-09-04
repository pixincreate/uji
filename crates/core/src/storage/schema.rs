// Diesel schema for the opencode-shaped session/message tables.
// Rust module names are plural (`sessions`, `messages`) to avoid clashing
// with the `session` domain module; SQL table names stay singular via
// `sql_name`. The `type` column is a Rust keyword, so it maps to `kind`.

diesel::table! {
    #[sql_name = "session"]
    sessions (id) {
        id -> Text,
        parent_id -> Nullable<Text>,
        title -> Text,
        directory -> Text,
        time_created -> BigInt,
        time_updated -> BigInt,
    }
}

diesel::table! {
    #[sql_name = "message"]
    messages (id) {
        id -> Text,
        session_id -> Text,
        seq -> BigInt,
        #[sql_name = "type"]
        kind -> Text,
        time_created -> BigInt,
        data -> Text,
    }
}

diesel::joinable!(messages -> sessions (session_id));
diesel::allow_tables_to_appear_in_same_query!(sessions, messages);
