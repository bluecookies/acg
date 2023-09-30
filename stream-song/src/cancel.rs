use tokio_util::sync::CancellationToken;
pub trait Cancellable {
    fn cancel_token(&self) -> CancellationToken;
}
