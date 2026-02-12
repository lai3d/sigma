use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub is_api_key: bool,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(format!("Invalid password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn create_token(
    user_id: Uuid,
    email: &str,
    role: &str,
    secret: &str,
    expiry_hours: u64,
) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        role: role.to_string(),
        iat: now,
        exp: now + (expiry_hours as i64 * 3600),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token creation failed: {e}")))
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}

pub fn require_role(user: &CurrentUser, allowed: &[&str]) -> Result<(), AppError> {
    if user.is_api_key || allowed.contains(&user.role.as_str()) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Role '{}' is not allowed. Required: {}",
            user.role,
            allowed.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let hash = hash_password("mypassword123").unwrap();
        assert!(verify_password("mypassword123", &hash).unwrap());
    }

    #[test]
    fn test_verify_wrong_password() {
        let hash = hash_password("correct").unwrap();
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn test_verify_invalid_hash() {
        let result = verify_password("anything", "not-a-valid-hash");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_verify_token() {
        let user_id = Uuid::new_v4();
        let token = create_token(user_id, "test@example.com", "admin", "secret123", 24).unwrap();
        let claims = verify_token(&token, "secret123").unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "admin");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_verify_expired_token() {
        let user_id = Uuid::new_v4();
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            email: "test@example.com".to_string(),
            role: "admin".to_string(),
            iat: now - 7200,
            exp: now - 3600, // expired 1 hour ago
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secret123"),
        )
        .unwrap();
        let result = verify_token(&token, "secret123");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_wrong_secret() {
        let user_id = Uuid::new_v4();
        let token = create_token(user_id, "test@example.com", "admin", "secret123", 24).unwrap();
        let result = verify_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_require_role_allowed() {
        let user = CurrentUser {
            id: Uuid::new_v4(),
            email: "admin@test.com".to_string(),
            role: "admin".to_string(),
            is_api_key: false,
        };
        assert!(require_role(&user, &["admin", "operator"]).is_ok());
    }

    #[test]
    fn test_require_role_denied() {
        let user = CurrentUser {
            id: Uuid::new_v4(),
            email: "reader@test.com".to_string(),
            role: "readonly".to_string(),
            is_api_key: false,
        };
        let result = require_role(&user, &["admin", "operator"]);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Forbidden(_) => {}
            other => panic!("Expected Forbidden, got {:?}", other),
        }
    }

    #[test]
    fn test_require_role_api_key_bypass() {
        let user = CurrentUser {
            id: Uuid::nil(),
            email: "api-key".to_string(),
            role: "admin".to_string(),
            is_api_key: true,
        };
        // Even if we check for a role that doesn't exist, API key bypasses
        assert!(require_role(&user, &["nonexistent-role"]).is_ok());
    }

    #[test]
    fn test_create_token_sets_expiry() {
        let user_id = Uuid::new_v4();
        let token = create_token(user_id, "test@example.com", "operator", "secret", 48).unwrap();
        let claims = verify_token(&token, "secret").unwrap();
        let expected_duration = 48 * 3600;
        let actual_duration = claims.exp - claims.iat;
        assert_eq!(actual_duration, expected_duration);
    }
}
