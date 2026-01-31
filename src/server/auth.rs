use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (agent name or identifier)
    pub sub: String,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Issuer
    pub iss: String,
}

impl Claims {
    #[allow(dead_code)]
    pub fn new(subject: &str, ttl_hours: i64) -> Self {
        let now = Utc::now();
        Self {
            sub: subject.to_string(),
            exp: (now + Duration::hours(ttl_hours)).timestamp(),
            iat: now.timestamp(),
            iss: "infractl".to_string(),
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }
}

pub struct JwtManager {
    #[allow(dead_code)]
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtManager {
    pub fn new(secret: &str) -> Self {
        let mut validation = Validation::default();
        validation.set_issuer(&["infractl"]);

        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            validation,
        }
    }

    #[allow(dead_code)]
    pub fn generate_token(&self, subject: &str, ttl_hours: i64) -> Result<String, JwtError> {
        let claims = Claims::new(subject, ttl_hours);
        encode(&Header::default(), &claims, &self.encoding_key).map_err(JwtError::Encode)
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map_err(JwtError::Decode)?;

        if token_data.claims.is_expired() {
            return Err(JwtError::Expired);
        }

        Ok(token_data.claims)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum JwtError {
    Encode(jsonwebtoken::errors::Error),
    Decode(jsonwebtoken::errors::Error),
    Expired,
    Missing,
    Invalid,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::Encode(e) => write!(f, "Failed to encode token: {}", e),
            JwtError::Decode(e) => write!(f, "Failed to decode token: {}", e),
            JwtError::Expired => write!(f, "Token has expired"),
            JwtError::Missing => write!(f, "Missing authorization token"),
            JwtError::Invalid => write!(f, "Invalid authorization header"),
        }
    }
}

impl std::error::Error for JwtError {}

/// Parse TTL string like "24h", "7d", "30m" to hours
#[allow(dead_code)]
pub fn parse_ttl_to_hours(ttl: &str) -> i64 {
    let ttl = ttl.trim();
    if ttl.is_empty() {
        return 24; // default
    }

    let (num_str, unit) = ttl.split_at(ttl.len() - 1);
    let num: i64 = num_str.parse().unwrap_or(24);

    match unit {
        "h" => num,
        "d" => num * 24,
        "m" => num / 60,
        "w" => num * 24 * 7,
        _ => 24,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_roundtrip() {
        let manager = JwtManager::new("test_secret");
        let token = manager.generate_token("agent-01", 24).unwrap();
        let claims = manager.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "agent-01");
    }

    #[test]
    fn test_parse_ttl() {
        assert_eq!(parse_ttl_to_hours("24h"), 24);
        assert_eq!(parse_ttl_to_hours("7d"), 168);
        assert_eq!(parse_ttl_to_hours("1w"), 168);
    }
}
