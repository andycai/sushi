use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    pub token_type: String,
}

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_ttl: i64,
    refresh_ttl: i64,
}

impl JwtService {
    pub fn new(secret: &str, access_ttl: i64, refresh_ttl: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_ttl,
            refresh_ttl,
        }
    }

    pub fn create_access_token(&self, user_id: i64, username: &str, role: &str) -> Result<String, String> {
        self.create_token(user_id, username, role, self.access_ttl, "access")
    }

    pub fn create_refresh_token(&self, user_id: i64, username: &str, role: &str) -> Result<String, String> {
        self.create_token(user_id, username, role, self.refresh_ttl, "refresh")
    }

    fn create_token(&self, user_id: i64, username: &str, role: &str, ttl: i64, token_type: &str) -> Result<String, String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            role: role.to_string(),
            exp: (now + Duration::seconds(ttl)).timestamp(),
            iat: now.timestamp(),
            token_type: token_type.to_string(),
        };
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| format!("token encode error: {e}"))
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, String> {
        let data = decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| format!("token decode error: {e}"))?;
        Ok(data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_verify_access_token() {
        let svc = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        let token = svc.create_access_token(1, "admin", "admin").unwrap();
        let claims = svc.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "1");
        assert_eq!(claims.username, "admin");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.token_type, "access");
    }

    #[test]
    fn test_refresh_token_type() {
        let svc = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        let token = svc.create_refresh_token(1, "user", "viewer").unwrap();
        let claims = svc.verify_token(&token).unwrap();
        assert_eq!(claims.token_type, "refresh");
    }

    #[test]
    fn test_invalid_token() {
        let svc = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        assert!(svc.verify_token("invalid.token.here").is_err());
    }
}
