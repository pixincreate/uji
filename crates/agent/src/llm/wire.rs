use std::time::Duration;

pub const STREAM_IDLE: Duration = Duration::from_secs(120);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

const TRANSPORT_IDLE: Duration = Duration::from_secs(STREAM_IDLE.as_secs() + 30);

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(TRANSPORT_IDLE)
        .build()
        .unwrap_or_default()
}
