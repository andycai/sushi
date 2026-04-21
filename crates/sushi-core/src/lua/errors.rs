#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaContractErrorCode {
    PublicPolicyConflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaContractError {
    code: LuaContractErrorCode,
    message: String,
}

impl LuaContractError {
    pub fn new(code: LuaContractErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub const fn code(&self) -> LuaContractErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
