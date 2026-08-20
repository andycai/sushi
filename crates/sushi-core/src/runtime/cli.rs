use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

type CliHandlerFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
type CliHandlerCallback = Arc<dyn Fn(Vec<String>) -> CliHandlerFuture + Send + Sync>;

static NEXT_CLI_HANDLER_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct CliHandler {
    id: u64,
    callback: CliHandlerCallback,
}

impl CliHandler {
    pub fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(Vec<String>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        Self {
            id: NEXT_CLI_HANDLER_ID.fetch_add(1, Ordering::Relaxed),
            callback: Arc::new(move |args| Box::pin(handler(args))),
        }
    }

    pub async fn call(&self, args: Vec<String>) -> Result<String, String> {
        (self.callback)(args).await
    }
}

impl std::fmt::Debug for CliHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CliHandler")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for CliHandler {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for CliHandler {}
