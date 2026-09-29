use tokio::task::{AbortHandle, JoinError};
use uji_native::{IntoReply, Reply};

pub(crate) struct Abort(pub(crate) AbortHandle);

impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Blocked<E> {
    #[error("{0}")]
    Failed(E),
    #[error("{0}")]
    Stopped(#[from] JoinError),
}

pub(crate) async fn blocking<T: Send + 'static, E: Send + 'static>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, Blocked<E>> {
    tokio::task::spawn_blocking(work)
        .await?
        .map_err(Blocked::Failed)
}

pub(crate) enum Line {
    Text(Vec<u8>),
    End,
    Late,
}

impl From<Option<Vec<u8>>> for Line {
    fn from(line: Option<Vec<u8>>) -> Self {
        line.map_or(Self::End, Self::Text)
    }
}

impl IntoReply for Line {
    fn into_reply(self) -> Reply {
        match self {
            Self::Text(text) => Reply::value(text),
            Self::End => Reply::end(),
            Self::Late => Reply::late(),
        }
    }
}
