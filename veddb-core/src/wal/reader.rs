//! WAL reader implementation for replay and recovery

use super::{WalEntry, WalError};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

/// WAL reader for replaying operations
pub struct WalReader {
    /// Buffered file reader
    reader: BufReader<File>,
    /// Path to the WAL file
    path: PathBuf,
    /// Number of entries read
    entries_read: u64,
}

impl WalReader {
    /// Open a WAL file for reading
    pub fn open(path: &Path) -> Result<Self, WalError> {
        let file = File::open(path).map_err(|e| {
            WalError::FileNotFound(format!("{}: {}", path.display(), e))
        })?;

        Ok(Self {
            reader: BufReader::new(file),
            path: path.to_path_buf(),
            entries_read: 0,
        })
    }

    /// Read the next entry from the WAL
    pub fn next_entry(&mut self) -> Result<Option<WalEntry>, WalError> {
        // Read entry length (4 bytes)
        let mut len_bytes = [0u8; 4];
        match self.reader.read_exact(&mut len_bytes) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // End of file reached
                return Ok(None);
            }
            Err(e) => return Err(WalError::IoError(e)),
        }

        let entry_len = u32::from_le_bytes(len_bytes) as usize;

        // Read entry data
        let mut entry_bytes = vec![0u8; entry_len];
        self.reader.read_exact(&mut entry_bytes)?;

        // Read checksum (4 bytes)
        let mut checksum_bytes = [0u8; 4];
        self.reader.read_exact(&mut checksum_bytes)?;
        let stored_checksum = u32::from_le_bytes(checksum_bytes);

        // Deserialize entry
        let mut entry: WalEntry = serde_json::from_slice(&entry_bytes)
            .map_err(|e| WalError::DeserializationError(e.to_string()))?;

        // Verify checksum
        entry.checksum = stored_checksum;
        if !entry.verify_checksum()? {
            return Err(WalError::CorruptedEntry(entry.sequence));
        }

        self.entries_read += 1;
        Ok(Some(entry))
    }

    /// Get the number of entries read so far
    pub fn entries_read(&self) -> u64 {
        self.entries_read
    }

    /// Get the path of the WAL file being read
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Scan a directory for all WAL files
pub fn scan_wal_files(wal_dir: &Path) -> Result<Vec<PathBuf>, WalError> {
    if !wal_dir.exists() {
        return Ok(Vec::new());
    }

    let mut wal_files = Vec::new();

    for entry in std::fs::read_dir(wal_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with("wal-") && file_name.ends_with(".log") {
                wal_files.push(path);
            }
        }
    }

    // Sort by file name (which includes the sequence number)
    wal_files.sort();

    Ok(wal_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::wal::{Operation, WalWriter, WalConfig, FsyncPolicy};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_wal_reader_basic() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            fsync_policy: FsyncPolicy::Always,
            ..Default::default()
        };

        // Write some entries
        let writer = WalWriter::new(config.clone()).unwrap();
        
        let op1 = Operation::Insert {
            collection: "users".to_string(),
            doc: Document::new(),
        };
        writer.append(op1).await.unwrap();

        let op2 = Operation::Delete {
            collection: "users".to_string(),
            id: crate::document::DocumentId::new(),
        };
        writer.append(op2).await.unwrap();

        writer.flush().await.unwrap();

        // Read entries back
        let wal_files = scan_wal_files(&config.wal_dir).unwrap();
        assert_eq!(wal_files.len(), 1);

        let mut reader = WalReader::open(&wal_files[0]).unwrap();

        let entry1 = reader.next_entry().unwrap();
        assert!(entry1.is_some());
        assert_eq!(entry1.unwrap().sequence, 0);

        let entry2 = reader.next_entry().unwrap();
        assert!(entry2.is_some());
        assert_eq!(entry2.unwrap().sequence, 1);

        let entry3 = reader.next_entry().unwrap();
        assert!(entry3.is_none());

        assert_eq!(reader.entries_read(), 2);
    }

    #[tokio::test]
    async fn test_scan_wal_files() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            max_file_size: 1024, // Small size to create multiple files
            ..Default::default()
        };

        let writer = WalWriter::new(config.clone()).unwrap();

        // Write many entries to create multiple files
        for _ in 0..100 {
            let op = Operation::Insert {
                collection: "test".to_string(),
                doc: Document::new(),
            };
            writer.append(op).await.unwrap();
        }

        writer.flush().await.unwrap();

        let wal_files = scan_wal_files(&config.wal_dir).unwrap();
        assert!(wal_files.len() > 1);

        // Files should be sorted
        for i in 1..wal_files.len() {
            assert!(wal_files[i - 1] < wal_files[i]);
        }
    }
}
