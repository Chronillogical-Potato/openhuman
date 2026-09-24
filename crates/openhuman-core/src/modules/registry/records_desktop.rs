//! Native desktop module. Published digests are added only from its release manifest.

use crate::modules::types::{LoadPolicy, ModuleRecord};
use tinydesktop_bus::names;

pub(crate) const TINYDESKTOP: ModuleRecord = ModuleRecord {
    id: "tinydesktop",
    description: "Permission-aware native desktop observation and control",
    bus_name: names::INTERFACE,
    object_path: names::OBJECT_PATH,
    version: "0.3.0",
    release_url: "https://github.com/tinyhumansai/tinydesktop/releases/tag/v0.3.0",
    // The local override may be tested before publication. Do not derive a
    // release pin from that debug build: use the published checksum.toml.
    assets: &[],
    load: LoadPolicy::Lazy,
};
