//! Placeholder for the TinySecurity `Callbacks2` host service.
//!
//! Callback authority stays unavailable until its owning caller migration lands.
//! Keeping the server seam explicit prevents an unimplemented callback from
//! being mistaken for consent, storage, or audit success.

/// Fail closed for callback operations not migrated to the bus yet.
pub(crate) fn unavailable() -> tinybus::Error {
    tinybus::Error::failed("security host callback unavailable")
}
