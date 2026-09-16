/// Cancellation for a turn or a job.
///
/// `tokio_util`'s token, under uji's name. The hand-rolled one this replaces
/// woke a timer every 50ms per waiter and reported cancellation that late.
pub use tokio_util::sync::CancellationToken as CancelToken;
