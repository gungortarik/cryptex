//! Cryptex shared domain foundations.
//!
//! M1 identity types follow accepted ADR-0015.
//! Durable sequence allocation is owned by `cryptexd` (not this crate).
//! OS secure-store (ADR-0014 ACCEPTED — P1) lives under cryptexd authority,
//! not in this shared domain crate.

pub mod brain;
pub mod error;
pub mod ids;
pub mod install;
pub mod pairing;
pub mod recovery_meta;
pub mod sequence;
pub mod validate;

pub use brain::{BRAIN_IDENTITY_SCHEMA_VERSION, BrainIdentity};
pub use error::{CoreError, CoreResult};
pub use ids::{DeviceId, InstallId};
pub use install::{INSTALL_IDENTITY_SCHEMA_VERSION, InstallIdentity};
pub use pairing::{PAIRING_RECORD_SCHEMA_VERSION, PairingRecord, PairingState};
pub use recovery_meta::{
    RECOVERY_KIT_PUBLIC_SCHEMA_VERSION, RecoveryKitPublicMetadata, RecoveryPurpose,
};
pub use sequence::DeviceSequenceCutoff;
