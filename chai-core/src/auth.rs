//! Authentication module for user registration, login, and JWT handling
//!
//! This module provides:
//! - User registration with email/password
//! - Login with JWT token generation
//! - Password hashing with Argon2
//! - JWT token validation

use crate::db;
use anyhow::{Context, Result};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT token expiration time in seconds (7 days)
const JWT_EXPIRATION_SECS: u64 = 7 * 24 * 60 * 60;

/// User model stored in the database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: i64,
}

/// Public user info (without password hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub email: String,
    pub created_at: i64,
}

impl From<User> for UserInfo {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            created_at: user.created_at,
        }
    }
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// User ID
    pub sub: i64,
    /// User email
    pub email: String,
    /// Expiration timestamp
    pub exp: u64,
    /// Issued at timestamp
    pub iat: u64,
}

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT secret key
    pub jwt_secret: String,
}

impl AuthConfig {
    /// Load config from environment variables
    pub fn from_env() -> Result<Self> {
        let jwt_secret =
            std::env::var("JWT_SECRET").context("JWT_SECRET environment variable not set")?;

        Ok(Self { jwt_secret })
    }
}

/// Hash a password using Argon2
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("Invalid password hash format: {}", e))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Generate a JWT token for a user
pub fn generate_token(user: &User, secret: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_secs();

    let claims = Claims {
        sub: user.id,
        email: user.email.clone(),
        exp: now + JWT_EXPIRATION_SECS,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .context("Failed to generate JWT token")
}

/// Validate a JWT token and return the claims
pub fn validate_token(token: &str, secret: &str) -> Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .context("Invalid or expired token")?;

    Ok(token_data.claims)
}

/// Register a new user
pub async fn register(email: &str, password: &str) -> Result<User> {
    // Validate email format (basic check)
    if !email.contains('@') || email.len() < 5 {
        anyhow::bail!("Invalid email format");
    }

    // Validate password strength
    if password.len() < 8 {
        anyhow::bail!("Password must be at least 8 characters long");
    }

    let pool = db::get_pool()?;

    // Check if email already exists
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await
        .context("Database error")?;

    if existing.is_some() {
        anyhow::bail!("Email already registered");
    }

    // Hash password
    let password_hash = hash_password(password)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_secs() as i64;

    // Insert user
    let result =
        sqlx::query("INSERT INTO users (email, password_hash, created_at) VALUES (?, ?, ?)")
            .bind(email)
            .bind(&password_hash)
            .bind(now)
            .execute(pool)
            .await
            .context("Failed to create user")?;

    let user = User {
        id: result.last_insert_rowid(),
        email: email.to_string(),
        password_hash,
        created_at: now,
    };

    tracing::info!("New user registered: {}", email);
    Ok(user)
}

/// Login a user and return a JWT token
pub async fn login(email: &str, password: &str, jwt_secret: &str) -> Result<(User, String)> {
    let pool = db::get_pool()?;

    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await
        .context("Database error")?;

    let user = user.ok_or_else(|| anyhow::anyhow!("Invalid email or password"))?;

    // Verify password
    if !verify_password(password, &user.password_hash)? {
        anyhow::bail!("Invalid email or password");
    }

    // Generate token
    let token = generate_token(&user, jwt_secret)?;

    tracing::info!("User logged in: {}", email);
    Ok((user, token))
}

/// Get user by ID
pub async fn get_user_by_id(user_id: i64) -> Result<Option<User>> {
    let pool = db::get_pool()?;

    sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Database error")
}

/// Get user by email
pub async fn get_user_by_email(email: &str) -> Result<Option<User>> {
    let pool = db::get_pool()?;

    sqlx::query_as("SELECT * FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await
        .context("Database error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();

        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_jwt_generation_and_validation() {
        let user = User {
            id: 1,
            email: "test@example.com".to_string(),
            password_hash: "fake_hash".to_string(),
            created_at: 0,
        };

        let secret = "test_secret_key_123";
        let token = generate_token(&user, secret).unwrap();

        let claims = validate_token(&token, secret).unwrap();
        assert_eq!(claims.sub, user.id);
        assert_eq!(claims.email, user.email);
    }

    #[test]
    fn test_jwt_validation_fails_with_wrong_secret() {
        let user = User {
            id: 1,
            email: "test@example.com".to_string(),
            password_hash: "fake_hash".to_string(),
            created_at: 0,
        };

        let token = generate_token(&user, "secret1").unwrap();
        assert!(validate_token(&token, "secret2").is_err());
    }
}
