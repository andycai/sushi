mod builtin;
mod cli;
mod http;
mod identity;
mod instance;
mod lifecycle;
mod migration;
mod profile;
mod registry;
mod task;

pub use builtin::{BuiltinFactoryRegistry, BuiltinPluginFactory};
pub use cli::CliHandler;
pub use http::{HttpHandler, HttpRequest, HttpResponse};
pub use identity::{PluginId, PluginInstanceId, RegistrationId};
pub use instance::{LuaRuntimeInstance, PluginHandle};
pub use lifecycle::PluginLifecycleState;
pub use migration::{
    historical_host_core_migrations, historical_menu_admin_migrations,
    historical_policy_migrations, load_lua_migrations, MigrationError, MigrationReport,
    MigrationReportEntry, MigrationRunner, MigrationStatus, MigrationVerificationEntry,
    MigrationVerificationStatus, PluginMigration,
};
pub use profile::{
    ProfileError, ResolvedRuntimeEntry, ResolvedRuntimeProfile, RuntimePluginSource,
    RuntimeProfileResolver,
};
pub use registry::{
    AdminPageSpec, CapabilityInspectionEntry, CapabilityRegistry, CapabilitySnapshot,
    CliCommandSpec, EventSubscriptionSpec, HttpRouteSpec, HttpSurface, MenuContributionSpec,
    OwnedRegistration, PendingCapabilityCommit, RegistrationConflict, RegistrationSource,
    StagedRegistrar, StaticRootSpec, TemplateRootSpec, TransportSpec,
};
pub(crate) use task::PendingTask;
pub use task::{PluginCancellationToken, TaskCancellationToken, TaskRegistration, TaskRegistry};
