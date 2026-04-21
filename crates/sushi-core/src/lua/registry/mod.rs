use crate::lua::contract::schema::api::ApiRouteContract;
use crate::lua::errors::{LuaContractError, LuaContractErrorCode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilitySnapshot {
    pub api_routes: Vec<ApiRouteContract>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    api_routes: Vec<ApiRouteContract>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_api(&mut self, route: ApiRouteContract) -> Result<(), LuaContractError> {
        if route.public && route.policy.is_some() {
            return Err(LuaContractError::new(
                LuaContractErrorCode::RegistrationDenied,
                "api route cannot set both public=true and policy",
            ));
        }

        self.api_routes.push(route);
        Ok(())
    }

    pub fn snapshot(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            api_routes: self.api_routes.clone(),
        }
    }
}
