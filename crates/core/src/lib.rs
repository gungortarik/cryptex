//! Cryptex shared domain foundations.
//!
//! M1 identity / sequence work follows accepted ADR-0015.
//! OS secure-store (ADR-0014 ACCEPTED — P1) lives under cryptexd authority,
//! not in this shared domain crate.

pub mod brain;
pub mod error;
pub mod ids;
pub mod pairing;
pub mod recovery_meta;
pub mod sequence;

pub use brain::{BRAIN_IDENTITY_SCHEMA_VERSION, BrainIdentity};
pub use error::{CoreError, CoreResult};
pub use ids::{DeviceId, InstallId};
pub use pairing::{PairingRecord, PairingState};
pub use recovery_meta::{
    RECOVERY_KIT_PUBLIC_SCHEMA_VERSION, RecoveryKitPublicMetadata, RecoveryPurpose,
};
pub use sequence::{COUNTER_SCHEMA_VERSION, DeviceSequenceCutoff, SequenceStore};
