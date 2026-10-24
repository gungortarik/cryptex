//! Bounded resource accounting for the M8 incoming-segment importer.
//!
//! These are importer admission limits, not canonical event-format limits.
//! A rejected batch remains untrusted in transport storage and never reaches
//! `canonical/events`.

/// Maximum sealed segments admitted in one prepared import transaction.
pub const MAX_IMPORT_FILES_PER_BATCH: usize = 16;

/// Maximum bytes in one staged sealed segment (1 GiB).
///
/// This accommodates 1,000 maximum-size ADR-0016 event records plus frame
/// headers while bounding one attacker-controlled file before it is copied.
pub const MAX_IMPORT_FILE_BYTES: u64 = 1_073_741_824;

/// Maximum aggregate staged bytes in one import transaction (4 GiB).
pub const MAX_IMPORT_BATCH_BYTES: u64 = 4_294_967_296;

/// At most one prepared import transaction may consume staging/journal space.
pub const MAX_PREPARED_IMPORT_TRANSACTIONS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportBatchBudget {
    files: usize,
    bytes: u64,
}

impl ImportBatchBudget {
    pub const fn empty() -> Self {
        Self { files: 0, bytes: 0 }
    }

    /// Reserve one candidate's declared length before opening/copying it.
    /// The caller must discard the whole batch if this returns an error.
    pub fn admit_file(&mut self, length: u64) -> Result<(), ImportLimitError> {
        if length == 0 {
            return Err(ImportLimitError::EmptyFile);
        }
        if length > MAX_IMPORT_FILE_BYTES {
            return Err(ImportLimitError::FileTooLarge {
                length,
                maximum: MAX_IMPORT_FILE_BYTES,
            });
        }
        let files = self
            .files
            .checked_add(1)
            .ok_or(ImportLimitError::TooManyFiles)?;
        if files > MAX_IMPORT_FILES_PER_BATCH {
            return Err(ImportLimitError::TooManyFiles);
        }
        let bytes = self
            .bytes
            .checked_add(length)
            .ok_or(ImportLimitError::BatchTooLarge)?;
        if bytes > MAX_IMPORT_BATCH_BYTES {
            return Err(ImportLimitError::BatchTooLarge);
        }
        self.files = files;
        self.bytes = bytes;
        Ok(())
    }

    pub const fn files(&self) -> usize {
        self.files
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl Default for ImportBatchBudget {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportLimitError {
    EmptyFile,
    FileTooLarge { length: u64, maximum: u64 },
    TooManyFiles,
    BatchTooLarge,
}

impl std::fmt::Display for ImportLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFile => write!(f, "empty sealed segment is not importable"),
            Self::FileTooLarge { length, maximum } => {
                write!(f, "import file length {length} exceeds maximum {maximum}")
            }
            Self::TooManyFiles => write!(
                f,
                "import batch exceeds maximum file count {MAX_IMPORT_FILES_PER_BATCH}"
            ),
            Self::BatchTooLarge => write!(
                f,
                "import batch exceeds maximum bytes {MAX_IMPORT_BATCH_BYTES}"
            ),
        }
    }
}

impl std::error::Error for ImportLimitError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_maximum_file_boundary() {
        let mut budget = ImportBatchBudget::empty();
        budget.admit_file(MAX_IMPORT_FILE_BYTES).unwrap();
        assert_eq!(budget.files(), 1);
        assert_eq!(budget.bytes(), MAX_IMPORT_FILE_BYTES);
    }

    #[test]
    fn refuses_file_over_limit_without_mutating_budget() {
        let mut budget = ImportBatchBudget::empty();
        assert_eq!(
            budget.admit_file(MAX_IMPORT_FILE_BYTES + 1),
            Err(ImportLimitError::FileTooLarge {
                length: MAX_IMPORT_FILE_BYTES + 1,
                maximum: MAX_IMPORT_FILE_BYTES,
            })
        );
        assert_eq!(budget, ImportBatchBudget::empty());
    }

    #[test]
    fn refuses_zero_length_files() {
        let mut budget = ImportBatchBudget::empty();
        assert_eq!(budget.admit_file(0), Err(ImportLimitError::EmptyFile));
        assert_eq!(budget, ImportBatchBudget::empty());
    }

    #[test]
    fn enforces_file_count_and_total_bytes_before_mutation() {
        let mut count_budget = ImportBatchBudget::empty();
        for _ in 0..MAX_IMPORT_FILES_PER_BATCH {
            count_budget.admit_file(1).unwrap();
        }
        let before = count_budget;
        assert_eq!(
            count_budget.admit_file(1),
            Err(ImportLimitError::TooManyFiles)
        );
        assert_eq!(count_budget, before);

        let mut byte_budget = ImportBatchBudget::empty();
        for _ in 0..4 {
            byte_budget.admit_file(MAX_IMPORT_FILE_BYTES).unwrap();
        }
        let before = byte_budget;
        assert_eq!(
            byte_budget.admit_file(1),
            Err(ImportLimitError::BatchTooLarge)
        );
        assert_eq!(byte_budget, before);
    }

    #[test]
    fn refuses_arithmetic_overflow() {
        let mut budget = ImportBatchBudget {
            files: 1,
            bytes: u64::MAX,
        };
        assert_eq!(budget.admit_file(1), Err(ImportLimitError::BatchTooLarge));
        assert_eq!(budget.bytes(), u64::MAX);
    }
}
