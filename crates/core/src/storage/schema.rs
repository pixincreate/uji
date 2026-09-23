diesel::table! {
    sessions (id) {
        id -> Text,
        title -> Text,
        directory -> Text,
        time_created -> BigInt,
        time_updated -> BigInt,
    }
}

diesel::table! {
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

diesel::table! {
    settings (key) {
        key -> Text,
        value -> Text,
    }
}
