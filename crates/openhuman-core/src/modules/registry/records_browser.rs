//! TinyBrowser remains a local-override-only module until release checksums exist.

use crate::modules::types::{LoadPolicy, ModuleRecord};

pub(crate) const TINYBROWSER: ModuleRecord = ModuleRecord {
    id: "tinybrowser",
    description: "Chrome browser automation",
    bus_name: tinybrowser_bus::names::INTERFACE,
    object_path: tinybrowser_bus::names::OBJECT_PATH,
    version: "0.2.1-dev",
    release_url: "",
    assets: &[],
    load: LoadPolicy::Lazy,
};
