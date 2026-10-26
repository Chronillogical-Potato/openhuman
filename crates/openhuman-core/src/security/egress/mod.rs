//! Egress spine (privacy epic S2, #4436).
//!
//! Centralizes "what leaves, to where, why" for every external data transfer.
//! See [`types`] for the [`EgressDescriptor`] contract and [`emit`] for the
//! single publish chokepoint. Consumed by the web bridge
//! ([`crate::web_chat`]) which surfaces the
//! descriptor to the frontend, and by later privacy slices (disclosure S3,
//! approval S4, identification-risk detector S5, enforcement S7).

pub mod emit;
pub mod privacy;
pub mod types;

pub use emit::{emit_external_transfer, emit_external_transfer_in, EgressLedger};
pub use privacy::{enforce_egress, local_only_tool_block, local_only_tool_block_blocking};
pub use types::{DataKind, EgressDescriptor, EgressReason, IdentificationRisk};
