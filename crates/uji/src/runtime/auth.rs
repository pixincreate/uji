use std::sync::Arc;

use uji_agent::auth::{self, AuthError};
use uji_agent::credential::{self, Credential};
use uji_agent::llm::Provider;

use super::signal::Signal;
use super::work::Work;

pub(crate) enum AuthEvent {
    Opened { url: String },
    Done { provider_id: String },
    Failed { message: String },
}

pub(crate) fn start(work: &Work, client: Arc<reqwest::Client>, provider: &Provider) {
    let config = provider.oauth.clone();
    let provider_id = provider.id.clone();

    work.stream(|signals| async move {
        let send = |event| {
            let _ = signals.send(Signal::Auth(event));
        };
        let Some(config) = config else {
            return send(AuthEvent::Failed {
                message: AuthError::Unsupported.to_string(),
            });
        };
        let pending = match auth::flow::start(&config) {
            Ok(pending) => pending,
            Err(err) => {
                return send(AuthEvent::Failed {
                    message: err.to_string(),
                });
            }
        };
        open_browser(&pending.url);
        send(AuthEvent::Opened {
            url: pending.url.clone(),
        });

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
        send(event);
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
