use std::fs;
use std::path::Path;

use nexa_core::error::{NexaError, Result};

const KEY_LEN: usize = 32;

/// Loads an existing master key from `data_dir/master.key`, or generates a new
/// one with a cryptographically-secure RNG and persists it to disk.
pub fn load_or_generate(data_dir: &Path) -> Result<[u8; KEY_LEN]> {
    let key_path = data_dir.join("master.key");
    if key_path.exists() {
        load_key(&key_path)
    } else {
        generate_key(&key_path)
    }
}

fn load_key(path: &Path) -> Result<[u8; KEY_LEN]> {
    let bytes =
        fs::read(path).map_err(|e| NexaError::Secret(format!("failed to read master key: {e}")))?;
    if bytes.len() != KEY_LEN {
        return Err(NexaError::Secret(format!(
            "master key invalid length: expected {KEY_LEN}, got {}",
            bytes.len()
        )));
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn generate_key(path: &Path) -> Result<[u8; KEY_LEN]> {
    use rand::RngCore;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| NexaError::Secret(format!("mkdir failed: {e}")))?;
    }

    let mut key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);

    fs::write(path, &key).map_err(|e| NexaError::Secret(format!("write key failed: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| NexaError::Secret(format!("set perms failed: {e}")))?;
    }

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_key_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let key = load_or_generate(dir.path()).unwrap();
        assert_eq!(key.len(), KEY_LEN);

        // Key file should exist on disk
        let key_path = dir.path().join("master.key");
        assert!(key_path.exists());
        assert_eq!(fs::read(&key_path).unwrap().len(), KEY_LEN);
    }

    #[test]
    fn loads_existing_key() {
        let dir = tempfile::tempdir().unwrap();
        let first = load_or_generate(dir.path()).unwrap();
        let second = load_or_generate(dir.path()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn rejects_invalid_key_length() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("master.key");
        fs::write(&key_path, b"tooshort").unwrap();

        let result = load_or_generate(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid length"), "unexpected error: {err}");
    }

    #[cfg(unix)]
    #[test]
    fn key_file_has_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        load_or_generate(dir.path()).unwrap();

        let key_path = dir.path().join("master.key");
        let perms = fs::metadata(&key_path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }
}
