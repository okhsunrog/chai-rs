//! Server-side authentication helpers

use anyhow::Result;
use chai_core::auth::{AuthConfig, Claims};

/// Validate JWT token and return claims
pub fn validate_token(token: &str) -> Result<Claims> {
    let config = AuthConfig::from_env()?;
    chai_core::auth::validate_token(token, &config.jwt_secret)
}

/// Check if token is valid (returns bool instead of Result)
pub fn is_token_valid(token: &str) -> bool {
    validate_token(token).is_ok()
}
