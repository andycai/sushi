#![allow(deprecated)]

pub mod event;

use std::sync::Arc;
use tokio::sync::Mutex;

/// Registered API route.
#[deprecated(note = "use runtime::OwnedRegistration<HttpRouteSpec>")]
pub struct RouteEntry {
    pub method: String,
    pub path: String,
}

/// Registered admin page.
#[deprecated(note = "use runtime::OwnedRegistration<AdminPageSpec>")]
pub struct AdminPageEntry {
    pub path: String,
    pub title: String,
}

/// Registered admin widget.
#[deprecated(note = "widgets will migrate to the owner-scoped runtime registry")]
pub struct AdminWidgetEntry {
    pub name: String,
}

/// Registered CLI command.
#[deprecated(note = "use runtime::OwnedRegistration<CliCommandSpec>")]
pub struct CliCommandEntry {
    pub name: String,
    pub description: String,
}

/// API route registry.
#[deprecated(note = "use runtime::CapabilityRegistry")]
#[derive(Default)]
pub struct ApiRegistry {
    pub routes: Arc<Mutex<Vec<RouteEntry>>>,
}

impl ApiRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_route(&self, method: &str, path: &str) {
        self.routes.lock().await.push(RouteEntry {
            method: method.to_string(),
            path: path.to_string(),
        });
    }

    pub async fn get_routes(&self) -> Vec<(String, String)> {
        let routes = self.routes.lock().await;
        routes
            .iter()
            .map(|r| (r.method.clone(), r.path.clone()))
            .collect()
    }
}

impl Clone for ApiRegistry {
    fn clone(&self) -> Self {
        Self {
            routes: Arc::clone(&self.routes),
        }
    }
}

/// Admin page/widget registry.
#[deprecated(note = "use runtime::CapabilityRegistry")]
#[derive(Default)]
pub struct AdminRegistry {
    pub pages: Arc<Mutex<Vec<AdminPageEntry>>>,
    pub widgets: Arc<Mutex<Vec<AdminWidgetEntry>>>,
}

impl AdminRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_page(&self, path: &str, title: &str) {
        self.pages.lock().await.push(AdminPageEntry {
            path: path.to_string(),
            title: title.to_string(),
        });
    }

    pub async fn register_widget(&self, name: &str) {
        self.widgets.lock().await.push(AdminWidgetEntry {
            name: name.to_string(),
        });
    }
}

impl Clone for AdminRegistry {
    fn clone(&self) -> Self {
        Self {
            pages: Arc::clone(&self.pages),
            widgets: Arc::clone(&self.widgets),
        }
    }
}

/// CLI command registry.
#[deprecated(note = "use runtime::CapabilityRegistry")]
#[derive(Default)]
pub struct CliRegistry {
    pub commands: Arc<Mutex<Vec<CliCommandEntry>>>,
}

impl CliRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_command(&self, name: &str, description: &str) {
        self.commands.lock().await.push(CliCommandEntry {
            name: name.to_string(),
            description: description.to_string(),
        });
    }

    pub async fn get_commands(&self) -> Vec<(String, String)> {
        let cmds = self.commands.lock().await;
        cmds.iter()
            .map(|c| (c.name.clone(), c.description.clone()))
            .collect()
    }
}

impl Clone for CliRegistry {
    fn clone(&self) -> Self {
        Self {
            commands: Arc::clone(&self.commands),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_registry() {
        let reg = ApiRegistry::new();
        reg.register_route("GET", "/api/hello").await;
        reg.register_route("POST", "/api/items").await;
        let routes = reg.get_routes().await;
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0], ("GET".to_string(), "/api/hello".to_string()));
    }

    #[tokio::test]
    async fn test_admin_registry() {
        let reg = AdminRegistry::new();
        reg.register_page("/admin/test", "Test Page").await;
        reg.register_widget("stats").await;
        assert_eq!(reg.pages.lock().await.len(), 1);
        assert_eq!(reg.widgets.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn test_cli_registry() {
        let reg = CliRegistry::new();
        reg.register_command("hello", "Say hello").await;
        let cmds = reg.get_commands().await;
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].0, "hello");
    }
}
