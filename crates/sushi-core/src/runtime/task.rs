use super::PluginInstanceId;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, watch, Mutex};
use tokio::task::AbortHandle;

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

type PendingTaskFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type PendingTaskFactory = Box<dyn FnOnce(TaskCancellationToken) -> PendingTaskFuture + Send>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRegistration {
    pub id: u64,
    pub owner: PluginInstanceId,
}

pub(crate) struct PendingTask {
    pub name: String,
    start: PendingTaskFactory,
}

impl PendingTask {
    pub(crate) fn new<F, Fut>(name: String, task: F) -> Self
    where
        F: FnOnce(TaskCancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self {
            name,
            start: Box::new(move |token| Box::pin(task(token))),
        }
    }
}

#[derive(Clone)]
pub struct PluginCancellationToken {
    cancel: watch::Sender<bool>,
    receiver: watch::Receiver<bool>,
}

impl PluginCancellationToken {
    pub(crate) fn new() -> Self {
        let (cancel, receiver) = watch::channel(false);
        Self { cancel, receiver }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn cancelled(&self) {
        let mut receiver = self.receiver.clone();
        if *receiver.borrow() {
            return;
        }
        while receiver.changed().await.is_ok() {
            if *receiver.borrow() {
                return;
            }
        }
    }

    pub(crate) fn cancel(&self) {
        let _ = self.cancel.send(true);
    }
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
        let (start, started) = oneshot::channel();
        let tasks = Arc::clone(&self.tasks);
        let task_owner = owner.clone();
        let mut guard = self.tasks.lock().await;
        let handle = tokio::spawn(async move {
            let _ = started.await;
            task(TaskCancellationToken { receiver }).await;
            remove_task(&tasks, &task_owner, id).await;
        });
        guard.entry(owner.clone()).or_default().push(TrackedTask {
            id,
            cancel,
            abort: handle.abort_handle(),
        });
        drop(guard);
        let _ = start.send(());

        TaskRegistration { id, owner }
    }

    pub(crate) async fn start_pending(
        &self,
        owner: PluginInstanceId,
        pending: PendingTask,
    ) -> TaskRegistration {
        self.spawn_owned(owner, move |token| (pending.start)(token))
            .await
    }

    pub async fn cancel_owner(&self, owner: &PluginInstanceId, timeout: Duration) {
        let registrations = self.registrations_for_owner(owner).await;
        self.cancel_registrations(&registrations, timeout).await;
    }

    pub async fn cancel_registrations(
        &self,
        registrations: &[TaskRegistration],
        timeout: Duration,
    ) {
        let targets = registrations
            .iter()
            .map(|registration| (registration.owner.clone(), registration.id))
            .collect::<HashSet<_>>();
        let controls = {
            let guard = self.tasks.lock().await;
            targets
                .iter()
                .filter_map(|(owner, id)| {
                    guard.get(owner).and_then(|tasks| {
                        tasks.iter().find(|task| task.id == *id).map(|task| {
                            (owner.clone(), *id, task.cancel.clone(), task.abort.clone())
                        })
                    })
                })
                .collect::<Vec<_>>()
        };
        for (_, _, cancel, _) in &controls {
            let _ = cancel.send(true);
        }

        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let active = {
                let guard = self.tasks.lock().await;
                targets.iter().any(|(owner, id)| {
                    guard
                        .get(owner)
                        .is_some_and(|tasks| tasks.iter().any(|task| task.id == *id))
                })
            };
            if !active {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        for (_, _, _, abort) in &controls {
            abort.abort();
        }
        if !controls.is_empty() {
            tracing::warn!(
                tasks = controls.len(),
                "plugin background tasks exceeded cancellation timeout and were aborted"
            );
        }
        let mut guard = self.tasks.lock().await;
        let owners = targets
            .iter()
            .map(|(owner, _)| owner.clone())
            .collect::<HashSet<_>>();
        for owner in owners {
            if let Some(tasks) = guard.get_mut(&owner) {
                tasks.retain(|task| !targets.contains(&(owner.clone(), task.id)));
                if tasks.is_empty() {
                    guard.remove(&owner);
                }
            }
        }
    }

    pub async fn cancel_all(&self, timeout: Duration) {
        let owners = self.tasks.lock().await.keys().cloned().collect::<Vec<_>>();
        for owner in owners {
            self.cancel_owner(&owner, timeout).await;
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

    pub async fn registrations_for_owner(&self, owner: &PluginInstanceId) -> Vec<TaskRegistration> {
        self.tasks
            .lock()
            .await
            .get(owner)
            .map(|tasks| {
                tasks
                    .iter()
                    .map(|task| TaskRegistration {
                        id: task.id,
                        owner: owner.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
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

    #[tokio::test]
    async fn owner_registrations_report_only_active_tasks() {
        let registry = TaskRegistry::new();
        let owner = PluginInstanceId::new("notes.default").unwrap();
        let registration = registry
            .spawn_owned(owner.clone(), |mut token| async move {
                token.cancelled().await;
            })
            .await;

        assert_eq!(
            registry.registrations_for_owner(&owner).await,
            vec![registration]
        );

        registry.cancel_owner(&owner, Duration::from_secs(1)).await;
        assert!(registry.registrations_for_owner(&owner).await.is_empty());
    }

    #[tokio::test]
    async fn cancelling_all_stops_tasks_for_every_owner() {
        let registry = TaskRegistry::new();
        let first_owner = PluginInstanceId::new("notes.default").unwrap();
        let second_owner = PluginInstanceId::new("cms.default").unwrap();
        registry
            .spawn_owned(first_owner.clone(), |mut token| async move {
                token.cancelled().await;
            })
            .await;
        registry
            .spawn_owned(second_owner.clone(), |mut token| async move {
                token.cancelled().await;
            })
            .await;

        registry.cancel_all(Duration::from_secs(1)).await;

        assert_eq!(registry.active_count(&first_owner).await, 0);
        assert_eq!(registry.active_count(&second_owner).await, 0);
    }

    #[tokio::test]
    async fn cancelling_registrations_keeps_newer_tasks_for_the_same_owner() {
        let registry = TaskRegistry::new();
        let owner = PluginInstanceId::new("notes.default").unwrap();
        let previous = registry
            .spawn_owned(owner.clone(), |mut token| async move {
                token.cancelled().await;
            })
            .await;
        let current = registry
            .spawn_owned(owner.clone(), |mut token| async move {
                token.cancelled().await;
            })
            .await;

        registry
            .cancel_registrations(&[previous], Duration::from_secs(1))
            .await;

        assert_eq!(
            registry.registrations_for_owner(&owner).await,
            vec![current]
        );
        registry.cancel_owner(&owner, Duration::from_secs(1)).await;
    }
}
