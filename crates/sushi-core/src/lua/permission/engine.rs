use crate::plugin::{DatabasePermission, Permissions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    ApiRoute,
    AdminPage,
    CliCommand,
    WebRender,
    DbRead,
    DbWrite,
    Event,
    Fs,
}

#[derive(Debug, Clone)]
pub struct PermissionDecisionEngine {
    permissions: Permissions,
    plugin_enabled: bool,
}

impl PermissionDecisionEngine {
    pub fn new(permissions: Permissions, plugin_enabled: bool) -> Self {
        Self {
            permissions,
            plugin_enabled,
        }
    }

    pub fn is_visible(&self, capability: CapabilityKind) -> bool {
        if !self.plugin_enabled {
            return false;
        }

        match capability {
            CapabilityKind::ApiRoute => self.permissions.routes,
            CapabilityKind::AdminPage => self.permissions.admin,
            CapabilityKind::CliCommand => self.permissions.commands,
            CapabilityKind::WebRender => self.permissions.admin || self.permissions.routes,
            CapabilityKind::DbRead => self.permissions.database != DatabasePermission::None,
            CapabilityKind::DbWrite => {
                matches!(
                    self.permissions.database,
                    DatabasePermission::Write | DatabasePermission::Admin
                )
            }
            CapabilityKind::Event => true,
            CapabilityKind::Fs => true,
        }
    }
}
