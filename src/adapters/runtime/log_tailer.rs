use std::path::Path;

use nexa_core::error::{NexaError, Result};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Reads log files and streams their contents line by line.
///
/// Used by `ContainerdRuntime` to stream logs that containerd writes to files
/// on the host filesystem (stdout/stderr redirected via `ctr tasks start`).
pub struct LogTailer;

impl LogTailer {
    /// Stream the last `tail` lines of a log file, then continue watching for
    /// new lines via a poll loop (250 ms sleep between polls).
    ///
    /// If `tail` is `None`, defaults to 100 lines of history.
    pub async fn tail(
        path: &Path,
        tail: Option<u64>,
    ) -> Result<impl futures::Stream<Item = Result<String>> + use<>> {
        let path = path.to_path_buf();
        if !path.exists() {
            return Err(NexaError::Runtime(format!(
                "log file not found: {}",
                path.display()
            )));
        }

        let n = tail.unwrap_or(100) as usize;

        // Read existing lines to serve the historical tail.
        let existing = Self::read_all_inner(&path).await?;
        let skip = existing.len().saturating_sub(n);
        let history: Vec<String> = existing.into_iter().skip(skip).collect();
        let byte_offset = {
            let meta = tokio::fs::metadata(&path)
                .await
                .map_err(|e| NexaError::Runtime(e.to_string()))?;
            meta.len()
        };

        let stream = async_stream::stream! {
            // Yield historical lines first.
            for line in history {
                yield Ok(line);
            }

            // Now follow the file for new lines.
            let file = match tokio::fs::File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    yield Err(NexaError::Runtime(e.to_string()));
                    return;
                }
            };

            let mut reader = BufReader::new(file);

            // Seek past the bytes we already served.
            {
                use tokio::io::AsyncSeekExt;
                if let Err(e) = reader.seek(std::io::SeekFrom::Start(byte_offset)).await {
                    yield Err(NexaError::Runtime(e.to_string()));
                    return;
                }
            }

            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => {
                        // No new data — wait and try again.
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    }
                    Ok(_) => {
                        let line = buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
                        if !line.is_empty() {
                            yield Ok(line);
                        }
                    }
                    Err(e) => {
                        yield Err(NexaError::Runtime(e.to_string()));
                        return;
                    }
                }
            }
        };

        Ok(stream)
    }

    /// Read all lines from a log file at once.
    pub async fn read_all(path: &Path) -> Result<Vec<String>> {
        Self::read_all_inner(path).await
    }

    async fn read_all_inner(path: &Path) -> Result<Vec<String>> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| NexaError::Runtime(format!("{}: {}", path.display(), e)))?;
        let reader = BufReader::new(file);
        let mut lines_stream = reader.lines();
        let mut lines = Vec::new();
        while let Some(line) = lines_stream
            .next_line()
            .await
            .map_err(|e| NexaError::Runtime(e.to_string()))?
        {
            lines.push(line);
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::io::Write;

    #[tokio::test]
    async fn tail_returns_last_n_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.log");
        {
            let mut f = std::fs::File::create(&log_path).unwrap();
            for i in 1..=20 {
                writeln!(f, "line {i}").unwrap();
            }
        }

        let stream = LogTailer::tail(&log_path, Some(5)).await.unwrap();
        tokio::pin!(stream);

        let mut collected = Vec::new();
        // Collect only the historical lines (first 5).
        for _ in 0..5 {
            if let Some(item) = stream.next().await {
                collected.push(item.unwrap());
            }
        }

        assert_eq!(collected.len(), 5);
        assert_eq!(collected[0], "line 16");
        assert_eq!(collected[4], "line 20");
    }

    #[tokio::test]
    async fn tail_returns_all_when_fewer_than_requested() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.log");
        {
            let mut f = std::fs::File::create(&log_path).unwrap();
            writeln!(f, "alpha").unwrap();
            writeln!(f, "beta").unwrap();
        }

        let stream = LogTailer::tail(&log_path, Some(100)).await.unwrap();
        tokio::pin!(stream);

        let mut collected = Vec::new();
        for _ in 0..2 {
            if let Some(item) = stream.next().await {
                collected.push(item.unwrap());
            }
        }

        assert_eq!(collected, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn tail_errors_on_missing_file() {
        let result = LogTailer::tail(Path::new("/tmp/nonexistent_nexad_test.log"), Some(10)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_all_returns_all_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.log");
        {
            let mut f = std::fs::File::create(&log_path).unwrap();
            writeln!(f, "one").unwrap();
            writeln!(f, "two").unwrap();
            writeln!(f, "three").unwrap();
        }

        let lines = LogTailer::read_all(&log_path).await.unwrap();
        assert_eq!(lines, vec!["one", "two", "three"]);
    }

    #[tokio::test]
    async fn tail_picks_up_new_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.log");
        {
            let mut f = std::fs::File::create(&log_path).unwrap();
            writeln!(f, "initial").unwrap();
        }

        let stream = LogTailer::tail(&log_path, Some(10)).await.unwrap();
        tokio::pin!(stream);

        // Consume the historical line.
        let first = stream.next().await.unwrap().unwrap();
        assert_eq!(first, "initial");

        // Append a new line to the file.
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&log_path)
                .unwrap();
            writeln!(f, "appended").unwrap();
        }

        // The stream should pick up the new line within a reasonable timeout.
        let new_line = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .expect("timed out waiting for new line")
            .unwrap()
            .unwrap();

        assert_eq!(new_line, "appended");
    }
}
