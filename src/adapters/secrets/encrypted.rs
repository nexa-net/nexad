use std::sync::Arc;

use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, KeyInit};
use async_trait::async_trait;
use rusqlite::Connection;
use tokio::sync::Mutex;

use nexa_core::error::{NexaError, Result};
use nexa_core::ports::secrets::SecretStore;

/// AES-256-GCM encrypted secret store backed by SQLite.
///
/// Ciphertext format stored in the DB: `nonce (12 bytes) || ciphertext`.
pub struct EncryptedSqliteSecretStore {
    conn: Arc<Mutex<Connection>>,
    cipher: Aes256Gcm,
}

impl EncryptedSqliteSecretStore {
    /// Create a new encrypted secret store.
    ///
    /// Initialises the `secrets` table if it does not already exist.
    pub fn new(conn: Connection, master_key: &[u8; 32]) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS secrets (
                project TEXT NOT NULL,
                name    TEXT NOT NULL,
                value   BLOB NOT NULL,
                PRIMARY KEY (project, name)
            )",
        )
        .map_err(|e| NexaError::Secret(format!("failed to init secrets table: {e}")))?;

        let cipher = Aes256Gcm::new_from_slice(master_key)
            .map_err(|e| NexaError::Secret(format!("invalid key: {e}")))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            cipher,
        })
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| NexaError::Secret(format!("encryption failed: {e}")))?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    fn decrypt(&self, blob: &[u8]) -> Result<Vec<u8>> {
        if blob.len() < 12 {
            return Err(NexaError::Secret("ciphertext too short".into()));
        }
        let (nonce_bytes, ciphertext) = blob.split_at(12);
        let nonce = aes_gcm::Nonce::from_slice(nonce_bytes);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| NexaError::Secret(format!("decryption failed: {e}")))
    }
}

#[async_trait]
impl SecretStore for EncryptedSqliteSecretStore {
    async fn set(&self, project: &str, name: &str, value: &[u8]) -> Result<()> {
        let encrypted = self.encrypt(value)?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO secrets (project, name, value) VALUES (?1, ?2, ?3)
             ON CONFLICT(project, name) DO UPDATE SET value = excluded.value",
            rusqlite::params![project, name, encrypted],
        )
        .map_err(|e| NexaError::Secret(format!("failed to set secret: {e}")))?;
        Ok(())
    }

    async fn get(&self, project: &str, name: &str) -> Result<Option<Vec<u8>>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT value FROM secrets WHERE project = ?1 AND name = ?2")
            .map_err(|e| NexaError::Secret(format!("query failed: {e}")))?;

        let result: std::result::Result<Vec<u8>, _> =
            stmt.query_row(rusqlite::params![project, name], |row| row.get(0));

        match result {
            Ok(blob) => {
                let plaintext = self.decrypt(&blob)?;
                Ok(Some(plaintext))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(NexaError::Secret(format!("query failed: {e}"))),
        }
    }

    async fn list(&self, project: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT name FROM secrets WHERE project = ?1 ORDER BY name")
            .map_err(|e| NexaError::Secret(format!("query failed: {e}")))?;

        let names = stmt
            .query_map(rusqlite::params![project], |row| row.get(0))
            .map_err(|e| NexaError::Secret(format!("query failed: {e}")))?
            .collect::<std::result::Result<Vec<String>, _>>()
            .map_err(|e| NexaError::Secret(format!("row read failed: {e}")))?;

        Ok(names)
    }

    async fn delete(&self, project: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM secrets WHERE project = ?1 AND name = ?2",
            rusqlite::params![project, name],
        )
        .map_err(|e| NexaError::Secret(format!("delete failed: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store(key: &[u8; 32]) -> EncryptedSqliteSecretStore {
        let conn = Connection::open_in_memory().unwrap();
        EncryptedSqliteSecretStore::new(conn, key).unwrap()
    }

    fn test_key() -> [u8; 32] {
        [0xAB; 32]
    }

    #[tokio::test]
    async fn set_and_get() {
        let store = make_store(&test_key());
        store.set("proj", "DB_PASS", b"s3cret").await.unwrap();
        let val = store.get("proj", "DB_PASS").await.unwrap();
        assert_eq!(val, Some(b"s3cret".to_vec()));
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let store = make_store(&test_key());
        let val = store.get("proj", "NOPE").await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn set_overwrites() {
        let store = make_store(&test_key());
        store.set("proj", "KEY", b"v1").await.unwrap();
        store.set("proj", "KEY", b"v2").await.unwrap();
        let val = store.get("proj", "KEY").await.unwrap();
        assert_eq!(val, Some(b"v2".to_vec()));
    }

    #[tokio::test]
    async fn list_sorted() {
        let store = make_store(&test_key());
        store.set("proj", "ZEBRA", b"z").await.unwrap();
        store.set("proj", "ALPHA", b"a").await.unwrap();
        store.set("proj", "MIDDLE", b"m").await.unwrap();
        let names = store.list("proj").await.unwrap();
        assert_eq!(names, vec!["ALPHA", "MIDDLE", "ZEBRA"]);
    }

    #[tokio::test]
    async fn list_empty() {
        let store = make_store(&test_key());
        let names = store.list("proj").await.unwrap();
        assert!(names.is_empty());
    }

    #[tokio::test]
    async fn delete() {
        let store = make_store(&test_key());
        store.set("proj", "KEY", b"val").await.unwrap();
        store.delete("proj", "KEY").await.unwrap();
        let val = store.get("proj", "KEY").await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_noop() {
        let store = make_store(&test_key());
        // Should not error
        store.delete("proj", "NOPE").await.unwrap();
    }

    #[tokio::test]
    async fn projects_isolated() {
        let store = make_store(&test_key());
        store.set("app1", "SECRET", b"value1").await.unwrap();
        store.set("app2", "SECRET", b"value2").await.unwrap();

        let v1 = store.get("app1", "SECRET").await.unwrap().unwrap();
        let v2 = store.get("app2", "SECRET").await.unwrap().unwrap();
        assert_eq!(v1, b"value1");
        assert_eq!(v2, b"value2");

        let names1 = store.list("app1").await.unwrap();
        assert_eq!(names1, vec!["SECRET"]);
    }

    #[tokio::test]
    async fn wrong_key_detection() {
        let store1 = make_store(&[0xAA; 32]);
        store1.set("proj", "KEY", b"secret").await.unwrap();

        // Read the raw encrypted blob from the DB
        let conn1 = store1.conn.lock().await;
        let blob: Vec<u8> = conn1
            .query_row(
                "SELECT value FROM secrets WHERE project = 'proj' AND name = 'KEY'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn1);

        // Create a new store with a different key, same DB data
        let conn2 = Connection::open_in_memory().unwrap();
        let store2 = EncryptedSqliteSecretStore::new(conn2, &[0xBB; 32]).unwrap();
        {
            let c = store2.conn.lock().await;
            c.execute(
                "INSERT INTO secrets (project, name, value) VALUES ('proj', 'KEY', ?1)",
                rusqlite::params![blob],
            )
            .unwrap();
        }

        let result = store2.get("proj", "KEY").await;
        assert!(result.is_err(), "decryption with wrong key should fail");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("decryption failed"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn binary_roundtrip() {
        let store = make_store(&test_key());
        let binary_data: Vec<u8> = (0..=255).collect();
        store.set("proj", "BIN", &binary_data).await.unwrap();
        let val = store.get("proj", "BIN").await.unwrap().unwrap();
        assert_eq!(val, binary_data);
    }
}
