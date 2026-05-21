use sha2::{Digest, Sha256};

use nexa_core::error::{NexaError, Result};

const TOKEN_PREFIX: &str = "nxa_";
const TOKEN_RANDOM_BYTES: usize = 32;

pub fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; TOKEN_RANDOM_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!("{}{}", TOKEN_PREFIX, hex::encode(bytes))
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn verify_token(token: &str, stored_hash: &str) -> bool {
    hash_token(token) == stored_hash
}

pub fn validate_token_format(token: &str) -> Result<()> {
    if !token.starts_with(TOKEN_PREFIX) {
        return Err(NexaError::Runtime("token must start with 'nxa_'".into()));
    }
    let hex_part = &token[TOKEN_PREFIX.len()..];
    if hex_part.len() != TOKEN_RANDOM_BYTES * 2 {
        return Err(NexaError::Runtime(format!(
            "token hex part must be {} characters, got {}",
            TOKEN_RANDOM_BYTES * 2,
            hex_part.len()
        )));
    }
    if hex::decode(hex_part).is_err() {
        return Err(NexaError::Runtime("token contains invalid hex".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_has_correct_format() {
        let token = generate_token();
        assert!(token.starts_with("nxa_"));
        assert_eq!(token.len(), 4 + 64);
        validate_token_format(&token).unwrap();
    }

    #[test]
    fn generate_token_is_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn hash_token_is_deterministic() {
        let token = "nxa_deadbeef";
        let h1 = hash_token(token);
        let h2 = hash_token(token);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn verify_token_correct() {
        let token = generate_token();
        let hash = hash_token(&token);
        assert!(verify_token(&token, &hash));
    }

    #[test]
    fn verify_token_wrong() {
        let token = generate_token();
        let hash = hash_token(&token);
        assert!(!verify_token("nxa_wrong", &hash));
    }

    #[test]
    fn validate_format_rejects_missing_prefix() {
        assert!(validate_token_format(
            "abc_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        )
        .is_err());
    }

    #[test]
    fn validate_format_rejects_short_hex() {
        assert!(validate_token_format("nxa_deadbeef").is_err());
    }

    #[test]
    fn validate_format_rejects_invalid_hex() {
        let bad = format!("nxa_{}", "g".repeat(64));
        assert!(validate_token_format(&bad).is_err());
    }
}
