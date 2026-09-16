use std::sync::Arc;

use uji_engine::auth::{self, AuthError};
use uji_engine::credential::{self, Credential};
use uji_engine::llm::Provider;

use super::signal::Signal;

pub(crate) enum AuthEvent {
    Opened { url: String },
    Done { provider_id: String },
    Failed { message: String },
}

pub(crate) fn start(
    runtime: &tokio::runtime::Runtime,
    client: Arc<reqwest::Client>,
    provider: &Provider,
    sender: calloop::channel::Sender<Signal>,
) {
    let Some(config) = provider.oauth.clone() else {
        let _ = sender.send(Signal::Auth(AuthEvent::Failed {
            message: AuthError::Unsupported.to_string(),
        }));
        return;
    };
    let provider_id = provider.id.clone();

    runtime.spawn(async move {
        let pending = match auth::flow::start(&config) {
            Ok(pending) => pending,
            Err(err) => {
                let _ = sender.send(Signal::Auth(AuthEvent::Failed {
                    message: err.to_string(),
                }));
                return;
            }
        };
        open_browser(&pending.url);
        let _ = sender.send(Signal::Auth(AuthEvent::Opened {
            url: pending.url.clone(),
        }));

        let event = match auth::login(&client, &config, pending).await {
            Ok(grant) => match credential::store(&provider_id, &Credential::from_grant(&grant)) {
                Ok(()) => AuthEvent::Done { provider_id },
                Err(err) => AuthEvent::Failed {
                    message: format!("signed in but could not save the credential: {err}"),
                },
            },
            Err(err) => AuthEvent::Failed {
                message: err.to_string(),
            },
        };
        let _ = sender.send(Signal::Auth(event));
    });
}

fn open_browser(url: &str) {
    let launcher = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(launcher)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
