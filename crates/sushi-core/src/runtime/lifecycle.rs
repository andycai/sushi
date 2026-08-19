#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginLifecycleState {
    Discovered,
    Resolved,
    Migrating,
    Activating,
    Active,
    Deactivating,
    Inactive,
    Failed,
}
