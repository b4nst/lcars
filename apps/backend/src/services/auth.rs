//! Authentication service for LCARS.
//!
//! Provides password hashing with Argon2 and JWT token management.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

/// JWT claims for authentication tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: i64,
    /// User role (admin or user)
    pub role: String,
    /// Expiration timestamp (Unix time)
    pub exp: usize,
    /// Issued at timestamp (Unix time)
    pub iat: usize,
}

/// Token expiration duration in seconds (24 hours).
const TOKEN_EXPIRATION_SECS: usize = 24 * 60 * 60;

/// Authentication service handling password hashing and JWT tokens.
pub struct AuthService {
    jwt_secret: String,
    argon2: Argon2<'static>,
}

impl AuthService {
    /// Creates a new AuthService with the given JWT secret.
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret,
            argon2: Argon2::default(),
        }
    }

    /// Hashes a password using Argon2.
    ///
    /// Returns the PHC-formatted hash string.
    pub fn hash_password(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(format!("Password hashing failed: {}", e)))?;
        Ok(hash.to_string())
    }

    /// Verifies a password against a stored hash.
    ///
    /// Returns true if the password matches.
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::Internal(format!("Invalid password hash format: {}", e)))?;

        Ok(self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Creates a JWT token for the given user.
    pub fn create_token(&self, user_id: i64, role: &str) -> Result<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| AppError::Internal(format!("System time error: {}", e)))?
            .as_secs() as usize;

        let claims = Claims {
            sub: user_id,
            role: role.to_string(),
            exp: now + TOKEN_EXPIRATION_SECS,
            iat: now,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Token creation failed: {}", e)))
    }

    /// Verifies a JWT token and returns the claims.
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| {
            tracing::debug!("Token verification failed: {}", e);
            AppError::Unauthorized
        })?;

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> AuthService {
        AuthService::new("test-secret-key-for-testing".to_string())
    }

    #[test]
    fn test_password_hash_and_verify() {
        let service = test_service();
        let password = "my-secure-password";

        let hash = service.hash_password(password).unwrap();

        // Hash should be in PHC format
        assert!(hash.starts_with("$argon2"));

        // Verification should succeed with correct password
        assert!(service.verify_password(password, &hash).unwrap());

        // Verification should fail with wrong password
        assert!(!service.verify_password("wrong-password", &hash).unwrap());
    }

    #[test]
    fn test_different_passwords_different_hashes() {
        let service = test_service();

        let hash1 = service.hash_password("password1").unwrap();
        let hash2 = service.hash_password("password2").unwrap();

        // Same password hashed twice should produce different hashes (due to salt)
        let hash3 = service.hash_password("password1").unwrap();

        assert_ne!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_token_create_and_verify() {
        let service = test_service();

        let token = service.create_token(42, "admin").unwrap();
        let claims = service.verify_token(&token).unwrap();

        assert_eq!(claims.sub, 42);
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_invalid_token_rejected() {
        let service = test_service();

        let result = service.verify_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_from_different_secret_rejected() {
        let service1 = AuthService::new("secret1".to_string());
        let service2 = AuthService::new("secret2".to_string());

        let token = service1.create_token(1, "user").unwrap();
        let result = service2.verify_token(&token);

        assert!(result.is_err());
    }
}
