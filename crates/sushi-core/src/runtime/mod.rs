mod http;
mod identity;
mod instance;
mod lifecycle;
mod migration;
mod profile;
mod registry;
mod task;

pub use http::{HttpHandler, HttpRequest, HttpResponse};
pub use identity::{PluginId, PluginInstanceId, RegistrationId};
pub use instance::LuaRuntimeInstance;
pub use lifecycle::PluginLifecycleState;
pub use migration::{
    historical_builtin_migrations, load_lua_migrations, MigrationError, MigrationReport,
    MigrationReportEntry, MigrationRunner, MigrationStatus, PluginMigration,
};
pub use profile::{
    ProfileError, ResolvedRuntimeEntry, ResolvedRuntimeProfile, RuntimePluginSource,
    RuntimeProfileResolver,
};
pub use registry::{
    AdminPageSpec, CapabilityInspectionEntry, CapabilityRegistry, CapabilitySnapshot,
    CliCommandSpec, EventSubscriptionSpec, HttpRouteSpec, HttpSurface, MenuContributionSpec,
    OwnedRegistration, PendingCapabilityCommit, RegistrationConflict, RegistrationSource,
    StagedRegistrar, StaticRootSpec, TemplateRootSpec,
};
pub use task::{TaskCancellationToken, TaskRegistration, TaskRegistry};
