//! WAL writer implementation

use super::{Operation, WalEntry, WalError};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Fsync policy for WAL writes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsyncPolicy {
    /// Fsync after every write (safest, slowest)
    Always,
    /// Fsync every second (balanced)
    EverySecond,
    /// No fsync (fastest, least safe)
    Disabled,
}

/// WAL writer configuration
#[derive(Debug, Clone)]
pub struct WalConfig {
    /// Directory for WAL files
    pub wal_dir: PathBuf,
    /// Fsync policy
    pub fsync_policy: FsyncPolicy,
    /// Maximum WAL file size before rotation (bytes)
    pub max_file_size: u64,
    /// Whether to compress WAL entries
    pub compress: bool,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            wal_dir: PathBuf::from("./data/wal"),
            fsync_policy: FsyncPolicy::EverySecond,
            max_file_size: 100 * 1024 * 1024, // 100 MB
            compress: false,
        }
    }
}

/// WAL writer for appending operations
pub struct WalWriter {
    /// Current file writer
    file: Arc<Mutex<BufWriter<File>>>,
    /// Current WAL file path
    current_file_path: Arc<Mutex<PathBuf>>,
    /// Current sequence number
    current_sequence: Arc<AtomicU64>,
    /// Configuration
    config: WalConfig,
    /// Current file size
    current_file_size: Arc<AtomicU64>,
    /// File number for rotation
    file_number: Arc<AtomicU64>,
}

impl WalWriter {
    /// Create a new WAL writer
    pub fn new(config: WalConfig) -> Result<Self, WalError> {
        // Create WAL directory if it doesn't exist
        std::fs::create_dir_all(&config.wal_dir)?;

        // Determine starting sequence and file number
        let (sequence, file_number) = Self::scan_existing_wals(&config.wal_dir)?;

        // Open or create the current WAL file
        let file_path = Self::wal_file_path(&config.wal_dir, file_number);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        let file_size = file.metadata()?.len();

        Ok(Self {
            file: Arc::new(Mutex::new(BufWriter::new(file))),
            current_file_path: Arc::new(Mutex::new(file_path)),
            current_sequence: Arc::new(AtomicU64::new(sequence)),
            config,
            current_file_size: Arc::new(AtomicU64::new(file_size)),
            file_number: Arc::new(AtomicU64::new(file_number)),
        })
    }

    /// Append an operation to the WAL
    pub async fn append(&self, operation: Operation) -> Result<u64, WalError> {
        let sequence = self.current_sequence.fetch_add(1, Ordering::SeqCst);

        // Create WAL entry
        let mut entry = WalEntry::new(sequence, operation);

        // Compute checksum
        entry.checksum = entry.compute_checksum()?;

        // Serialize entry
        let bytes = self.serialize_entry(&entry)?;

        // Check if we need to rotate
        if self.should_rotate(bytes.len()).await {
            self.rotate_file().await?;
        }

        // Write to file
        {
            let mut file = self.file.lock().await;
            file.write_all(&bytes)?;

            // Fsync based on policy
            if self.config.fsync_policy == FsyncPolicy::Always {
                file.flush()?;
                file.get_ref().sync_all()?;
            }
        }

        // Update file size
        self.current_file_size
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);

        Ok(sequence)
    }

    /// Flush the WAL to disk
    pub async fn flush(&self) -> Result<(), WalError> {
        let mut file = self.file.lock().await;
        file.flush()?;
        file.get_ref().sync_all()?;
        Ok(())
    }

    /// Get the current sequence number
    pub fn current_sequence(&self) -> u64 {
        self.current_sequence.load(Ordering::Relaxed)
    }

    /// Start background fsync task (for EverySecond policy)
    pub async fn start_background_fsync(self: Arc<Self>) {
        if self.config.fsync_policy != FsyncPolicy::EverySecond {
            return;
        }

        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(1)).await;

                if let Err(e) = self.flush().await {
                    eprintln!("Background fsync error: {}", e);
                }
            }
        });
    }

    /// Serialize a WAL entry to bytes
    fn serialize_entry(&self, entry: &WalEntry) -> Result<Vec<u8>, WalError> {
        // Format: [length(4) | entry_bytes | checksum(4)]
        // Use serde_json instead of bincode for better compatibility with BTreeMap
        let entry_json = serde_json::to_vec(entry)
            .map_err(|e| WalError::SerializationError(e.to_string()))?;

        let mut bytes = Vec::with_capacity(4 + entry_json.len() + 4);

        // Write length
        bytes.extend_from_slice(&(entry_json.len() as u32).to_le_bytes());

        // Write entry
        bytes.extend_from_slice(&entry_json);

        // Write checksum
        bytes.extend_from_slice(&entry.checksum.to_le_bytes());

        Ok(bytes)
    }

    /// Check if we should rotate to a new file
    async fn should_rotate(&self, next_entry_size: usize) -> bool {
        let current_size = self.current_file_size.load(Ordering::Relaxed);
        current_size + next_entry_size as u64 > self.config.max_file_size
    }

    /// Rotate to a new WAL file
    async fn rotate_file(&self) -> Result<(), WalError> {
        // Flush current file
        self.flush().await?;

        // Increment file number
        let new_file_number = self.file_number.fetch_add(1, Ordering::SeqCst) + 1;

        // Create new file path
        let new_file_path = Self::wal_file_path(&self.config.wal_dir, new_file_number);

        // Open new file
        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&new_file_path)?;

        // Replace file and path
        {
            let mut file = self.file.lock().await;
            *file = BufWriter::new(new_file);
        }

        {
            let mut path = self.current_file_path.lock().await;
            *path = new_file_path;
        }

        // Reset file size
        self.current_file_size.store(0, Ordering::Relaxed);

        Ok(())
    }

    /// Generate WAL file path
    fn wal_file_path(wal_dir: &Path, file_number: u64) -> PathBuf {
        wal_dir.join(format!("wal-{:010}.log", file_number))
    }

    /// Scan existing WAL files to determine starting sequence and file number.
    /// 
    /// INVARIANT: WAL sequence numbers are GLOBAL and STRICTLY MONOTONIC (not per-file).
    /// This function scans all WAL files to find the maximum sequence number used,
    /// ensuring new entries continue with a globally unique sequence.
    fn scan_existing_wals(wal_dir: &Path) -> Result<(u64, u64), WalError> {
        use super::reader::WalReader;
        
        if !wal_dir.exists() {
            return Ok((0, 0));
        }

        let mut max_file_number = 0u64;
        let mut global_max_sequence = 0u64;

        for entry in std::fs::read_dir(wal_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("wal-") && file_name.ends_with(".log") {
                    // Extract file number
                    if let Some(num_str) = file_name.strip_prefix("wal-").and_then(|s| s.strip_suffix(".log")) {
                        if let Ok(file_num) = num_str.parse::<u64>() {
                            max_file_number = max_file_number.max(file_num);

                            // Scan file for max sequence (enforces global monotonic invariant)
                            match Self::scan_file_for_max_sequence(&path) {
                                Ok(file_max_seq) => {
                                    global_max_sequence = global_max_sequence.max(file_max_seq);
                                }
                                Err(WalError::FileNotFound(_)) => {
                                    // File was deleted between scan and read, skip it
                                    continue;
                                }
                                Err(e) => {
                                    // Log warning but continue - file might be corrupted
                                    eprintln!("Warning: Failed to scan WAL file {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Next sequence is one more than the max found
        let next_sequence = if global_max_sequence > 0 {
            global_max_sequence + 1
        } else {
            0
        };

        Ok((next_sequence, max_file_number))
    }

    /// Scan a single WAL file to find the maximum sequence number it contains.
    /// Returns 0 if file is empty or unreadable.
    fn scan_file_for_max_sequence(path: &Path) -> Result<u64, WalError> {
        use super::reader::WalReader;
        
        let mut reader = WalReader::open(path)?;
        let mut max_seq = 0u64;

        loop {
            match reader.next_entry() {
                Ok(Some(entry)) => {
                    max_seq = max_seq.max(entry.sequence);
                }
                Ok(None) => {
                    // End of file
                    break;
                }
                Err(WalError::CorruptedEntry(seq)) => {
                    // Skip corrupted entry but record sequence if valid
                    max_seq = max_seq.max(seq);
                    // Continue reading - may find more valid entries
                    continue;
                }
                Err(_) => {
                    // Stop on other errors (IO error, deserialization error)
                    break;
                }
            }
        }

        Ok(max_seq)
    }

    /// Compact old WAL files (remove files where ALL entries have sequence < before_sequence).
    /// 
    /// Only removes files that contain ONLY entries with sequence numbers strictly less than
    /// the specified threshold. This ensures no data loss during compaction.
    pub async fn compact(&self, before_sequence: u64) -> Result<usize, WalError> {
        let mut removed_count = 0;
        let current_file_num = self.file_number.load(Ordering::Relaxed);

        for entry in std::fs::read_dir(&self.config.wal_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("wal-") && file_name.ends_with(".log") {
                    if let Some(num_str) = file_name.strip_prefix("wal-").and_then(|s| s.strip_suffix(".log")) {
                        if let Ok(file_num) = num_str.parse::<u64>() {
                            // Never remove the current file
                            if file_num >= current_file_num {
                                continue;
                            }

                            // Check if file contains only entries before the sequence
                            match Self::file_contains_only_entries_before(&path, before_sequence) {
                                Ok(true) => {
                                    // Safe to remove - all entries are before threshold
                                    if let Err(e) = std::fs::remove_file(&path) {
                                        eprintln!("Warning: Failed to remove WAL file {:?}: {}", path, e);
                                    } else {
                                        removed_count += 1;
                                    }
                                }
                                Ok(false) => {
                                    // File contains entries >= threshold, keep it
                                }
                                Err(e) => {
                                    // Can't validate file, keep it to be safe
                                    eprintln!("Warning: Failed to validate WAL file {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }

    /// Check if a WAL file contains only entries with sequence < threshold.
    /// Returns true if ALL entries have sequence < before_sequence (safe to delete).
    /// Returns false if ANY entry has sequence >= before_sequence (must keep).
    fn file_contains_only_entries_before(path: &Path, before_sequence: u64) -> Result<bool, WalError> {
        use super::reader::WalReader;
        
        let mut reader = WalReader::open(path)?;

        loop {
            match reader.next_entry() {
                Ok(Some(entry)) => {
                    // If any entry is at or after threshold, file must be kept
                    if entry.sequence >= before_sequence {
                        return Ok(false);
                    }
                }
                Ok(None) => {
                    // Reached end of file - all entries were before threshold
                    break;
                }
                Err(WalError::CorruptedEntry(seq)) => {
                    // Even corrupted entries count - check their sequence
                    if seq >= before_sequence {
                        return Ok(false);
                    }
                    continue;
                }
                Err(_) => {
                    // Can't read file properly - return error to be safe
                    return Err(WalError::IoError(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Failed to fully read WAL file for validation"
                    )));
                }
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, DocumentId};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_wal_writer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let writer = WalWriter::new(config).unwrap();
        assert_eq!(writer.current_sequence(), 0);
    }

    #[tokio::test]
    async fn test_wal_append() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            fsync_policy: FsyncPolicy::Always,
            ..Default::default()
        };

        let writer = WalWriter::new(config).unwrap();

        let op = Operation::Insert {
            collection: "users".to_string(),
            doc: Document::new(),
        };

        let seq = writer.append(op).await.unwrap();
        assert_eq!(seq, 0);

        let seq2 = writer.append(Operation::Delete {
            collection: "users".to_string(),
            id: DocumentId::new(),
        }).await.unwrap();
        assert_eq!(seq2, 1);
    }

    #[tokio::test]
    async fn test_wal_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            max_file_size: 1024, // Small size to trigger rotation
            ..Default::default()
        };

        let writer = WalWriter::new(config).unwrap();

        // Write many entries to trigger rotation
        for _ in 0..100 {
            let op = Operation::Insert {
                collection: "test".to_string(),
                doc: Document::new(),
            };
            writer.append(op).await.unwrap();
        }

        // Check that multiple files were created
        let file_count = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_str()
                    .unwrap()
                    .starts_with("wal-")
            })
            .count();

        assert!(file_count > 1);
    }
}
