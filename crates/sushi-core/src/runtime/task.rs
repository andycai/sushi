use super::PluginInstanceId;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Mutex};
use tokio::task::AbortHandle;

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRegistration {
    pub id: u64,
    pub owner: PluginInstanceId,
}

#[derive(Clone)]
pub struct TaskCancellationToken {
    receiver: watch::Receiver<bool>,
}

impl TaskCancellationToken {
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn cancelled(&mut self) {
        if self.is_cancelled() {
            return;
        }
        while self.receiver.changed().await.is_ok() {
            if self.is_cancelled() {
                return;
            }
        }
    }
}

struct TrackedTask {
    id: u64,
    cancel: watch::Sender<bool>,
    abort: AbortHandle,
}

type TasksByOwner = HashMap<PluginInstanceId, Vec<TrackedTask>>;

#[derive(Clone, Default)]
pub struct TaskRegistry {
    tasks: Arc<Mutex<TasksByOwner>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn spawn_owned<F, Fut>(&self, owner: PluginInstanceId, task: F) -> TaskRegistration
    where
        F: FnOnce(TaskCancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let (cancel, receiver) = watch::channel(false);
        let tasks = Arc::clone(&self.tasks);
        let task_owner = owner.clone();
        let mut guard = self.tasks.lock().await;
        let handle = tokio::spawn(async move {
            task(TaskCancellationToken { receiver }).await;
            remove_task(&tasks, &task_owner, id).await;
        });
        guard.entry(owner.clone()).or_default().push(TrackedTask {
            id,
            cancel,
            abort: handle.abort_handle(),
        });
        drop(guard);

        TaskRegistration { id, owner }
    }

    pub async fn cancel_owner(&self, owner: &PluginInstanceId, timeout: Duration) {
        let controls = {
            let guard = self.tasks.lock().await;
            guard
                .get(owner)
                .map(|tasks| {
                    tasks
                        .iter()
                        .map(|task| (task.id, task.cancel.clone(), task.abort.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        for (_, cancel, _) in &controls {
            let _ = cancel.send(true);
        }

        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.active_count(owner).await == 0 {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        for (_, _, abort) in &controls {
            abort.abort();
        }
        if !controls.is_empty() {
            tracing::warn!(
                owner = %owner,
                tasks = controls.len(),
                "plugin background tasks exceeded cancellation timeout and were aborted"
            );
        }
        let ids = controls
            .into_iter()
            .map(|(id, _, _)| id)
            .collect::<Vec<_>>();
        let mut guard = self.tasks.lock().await;
        if let Some(tasks) = guard.get_mut(owner) {
            tasks.retain(|task| !ids.contains(&task.id));
            if tasks.is_empty() {
                guard.remove(owner);
            }
        }
    }

    pub async fn active_count(&self, owner: &PluginInstanceId) -> usize {
        self.tasks
            .lock()
            .await
            .get(owner)
            .map(Vec::len)
            .unwrap_or(0)
    }
}

async fn remove_task(tasks: &Arc<Mutex<TasksByOwner>>, owner: &PluginInstanceId, id: u64) {
    let mut guard = tasks.lock().await;
    if let Some(owner_tasks) = guard.get_mut(owner) {
        owner_tasks.retain(|task| task.id != id);
        if owner_tasks.is_empty() {
            guard.remove(owner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn cancelling_owner_stops_only_owned_tasks() {
        let registry = TaskRegistry::new();
        let first_owner = PluginInstanceId::new("notes.default").unwrap();
        let second_owner = PluginInstanceId::new("cms.default").unwrap();
        let first_cancelled = Arc::new(AtomicBool::new(false));
        let second_cancelled = Arc::new(AtomicBool::new(false));

        let observed = Arc::clone(&first_cancelled);
        registry
            .spawn_owned(first_owner.clone(), move |mut token| async move {
                token.cancelled().await;
                observed.store(true, Ordering::SeqCst);
            })
            .await;
        let observed = Arc::clone(&second_cancelled);
        registry
            .spawn_owned(second_owner.clone(), move |mut token| async move {
                token.cancelled().await;
                observed.store(true, Ordering::SeqCst);
            })
            .await;

        registry
            .cancel_owner(&first_owner, Duration::from_secs(1))
            .await;

        assert!(first_cancelled.load(Ordering::SeqCst));
        assert!(!second_cancelled.load(Ordering::SeqCst));
        assert_eq!(registry.active_count(&first_owner).await, 0);
        assert_eq!(registry.active_count(&second_owner).await, 1);

        registry
            .cancel_owner(&second_owner, Duration::from_secs(1))
            .await;
    }

    #[tokio::test]
    async fn uncooperative_task_is_aborted_after_timeout() {
        let registry = TaskRegistry::new();
        let owner = PluginInstanceId::new("stuck.default").unwrap();
        registry
            .spawn_owned(owner.clone(), |_| async {
                std::future::pending::<()>().await;
            })
            .await;

        registry.cancel_owner(&owner, Duration::ZERO).await;
        assert_eq!(registry.active_count(&owner).await, 0);
    }
}
