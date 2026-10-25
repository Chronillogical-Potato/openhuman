//! Registry metadata against each module's bus contract constants.

use super::{component_for, find, ALL, COMPONENT_OF};

/// The interface and object path each contract crate publishes, keyed by the
/// module id the registry uses. This is written out from the contract
/// constants, not from the records, so a record that drifts from its contract
/// (or a contract that is renamed upstream) fails here rather than at the first
/// bus call. Records behind an optional contract crate are listed under the
/// same gate as the crate.
fn contract_names() -> Vec<(&'static str, &'static str, &'static str)> {
    #[allow(unused_mut)]
    let mut names = vec![
        (
            "tinycomputer",
            tinycomputer_bus::names::INTERFACE,
            tinycomputer_bus::names::OBJECT_PATH,
        ),
        (
            "tinysearch",
            tinysearch_bus::names::INTERFACE,
            tinysearch_bus::names::OBJECT_PATH,
        ),
        (
            "tinyjuice",
            tinyjuice_bus::names::BUS_NAME,
            tinyjuice_bus::names::OBJECT_PATH,
        ),
        (
            "tinymcp",
            tinymcp_bus::names::INTERFACE,
            tinymcp_bus::names::OBJECT_PATH,
        ),
        (
            "tinyconnectors",
            tinyconnectors_bus::names::INTERFACE,
            tinyconnectors_bus::names::OBJECT_PATH,
        ),
        (
            "tinychannels",
            tinychannels_bus::names::BUS_NAME,
            tinychannels_bus::names::OBJECT_PATH,
        ),
        // The Authoring identity the module registers under; the scheduler is
        // a second object on it (pinned by `the_flows_record_serves_the_schedule_object`).
        (
            "tinyflows",
            tinyflows_bus::BUS_NAME,
            tinyflows_bus::OBJECT_PATH,
        ),
    ];
    #[cfg(feature = "modules")]
    names.push(("tinybox", tinybox_bus::INTERFACE, tinybox_bus::OBJECT_PATH));
    #[cfg(feature = "documents")]
    names.push((
        "tinydocs",
        tinydocs_bus::names::BUS_NAME,
        tinydocs_bus::names::OBJECT_PATH,
    ));
    #[cfg(feature = "web3")]
    names.push((
        "tinywallet",
        tinywallet_bus::names::BUS_NAME,
        tinywallet_bus::names::OBJECT_PATH,
    ));
    #[cfg(feature = "voice")]
    names.push((
        "tinyvoice",
        tinyvoice_bus::names::BUS_NAME,
        tinyvoice_bus::names::OBJECT_PATH,
    ));
    #[cfg(feature = "hosting")]
    names.push((
        "tinyhosts",
        tinyhosts_bus::BUS_NAME,
        tinyhosts_bus::OBJECT_PATH,
    ));
    #[cfg(feature = "inbound-queues")]
    names.push((
        "tinyeventqueues",
        tinyeventqueues_bus::INTERFACE,
        tinyeventqueues_bus::OBJECT_PATH,
    ));
    names
}

#[test]
fn every_record_matches_its_contract_constants() {
    let contracts = contract_names();
    assert_eq!(contracts.len(), ALL.len(), "a record has no contract entry");
    for (id, bus_name, object_path) in contracts {
        let record = find(id).unwrap_or_else(|| panic!("{id} has no registry record"));
        assert_eq!(
            record.bus_name, bus_name,
            "{id} bus name drifted from its contract"
        );
        assert_eq!(
            record.object_path, object_path,
            "{id} object path drifted from its contract"
        );
    }
}

#[test]
fn a_drifted_contract_constant_is_reported_as_a_mismatch() {
    // The comparison must be able to fail: a fixture whose constant differs
    // from the record (as a renamed upstream interface would) is not equal.
    let record = find("tinymcp").unwrap();
    let drifted = format!("{}Renamed", tinymcp_bus::names::INTERFACE);
    assert_ne!(record.bus_name, drifted);
}

#[cfg(all(
    feature = "documents",
    feature = "web3",
    feature = "voice",
    feature = "hosting",
    feature = "inbound-queues"
))]
#[test]
fn a_product_feature_build_carries_all_thirteen_records() {
    assert_eq!(ALL.len(), 13);
    for id in [
        "tinycomputer",
        "tinysearch",
        "tinydocs",
        "tinywallet",
        "tinyjuice",
        "tinyvoice",
        "tinymcp",
        "tinyconnectors",
        "tinybox",
        "tinychannels",
        "tinyflows",
        "tinyhosts",
        "tinyeventqueues",
    ] {
        assert!(find(id).is_some(), "{id} missing from the registry");
    }
}

#[test]
fn every_record_maps_to_a_real_component() {
    for record in ALL {
        // TinySecurity has no product component yet; the global module switch
        // is its only host gate until its callers move behind the bus.
        if record.id == "tinysecurity" {
            assert_eq!(component_for(record.id), None);
            continue;
        }
        let component = component_for(record.id)
            .unwrap_or_else(|| panic!("module '{}' has no component mapping", record.id));
        assert!(
            crate::core::runtime::COMPONENT_NAMES.contains(&component),
            "'{component}' is not a [components] name"
        );
    }
    for (id, _) in COMPONENT_OF {
        assert!(
            ALL.iter().any(|r| r.id == *id)
                || !cfg!(feature = "modules")
                || matches!(
                    *id,
                    "tinydocs" | "tinywallet" | "tinyvoice" | "tinyhosts" | "tinyeventqueues"
                ),
            "mapping for unknown module '{id}'"
        );
    }
}

/// The scheduler is a second interface on the same module, and the host calls
/// it by the contract's own names: the record carries the Authoring identity,
/// and the Schedule object and callback interfaces are distinct from it and from
/// each other.
#[test]
fn the_flows_record_serves_the_schedule_object() {
    use tinyflows_bus::schedule;
    let record = find("tinyflows").expect("tinyflows is registered");
    assert_eq!(record.bus_name, schedule::BUS_NAME);
    assert_ne!(record.object_path, schedule::OBJECT_PATH);
    assert_ne!(schedule::INTERFACE, schedule::CALLBACK_INTERFACE);
    assert_ne!(schedule::INTERFACE, schedule::DOCUMENT_INTERFACE);
    assert_eq!(schedule::CONTRACT_VERSION, 8, "the host speaks contract v8");
    assert_eq!(component_for("tinyflows"), Some("tinyflows"));
}

/// The pins are copied from the published v0.9.0 `checksum.toml`.
#[test]
fn the_flows_record_pins_every_host_of_one_release() {
    let record = find("tinyflows").unwrap();
    assert_eq!(record.version, "0.9.0");
    assert_eq!(record.assets.len(), 11);
    let expected = [
        (
            "tinyflows-module-0.9.0-macos-26-arm64.tar.gz",
            "5b3747231ad717342a55fd51f50e51b7a0d917ea776a5631d361030b6dce53d2",
        ),
        (
            "tinyflows-module-0.9.0-macos-26-x86_64.tar.gz",
            "8c51d3469a449a5819fee44f359458bc890f3af77e00318d40b6fc529e2e78e6",
        ),
        (
            "tinyflows-module-0.9.0-macos-15-arm64.tar.gz",
            "13699ad57e71a90245a889a0d3168cd2996d495d45fa8be119806396f26bfe06",
        ),
        (
            "tinyflows-module-0.9.0-macos-15-x86_64.tar.gz",
            "4c5ee2bc248551916ba9f8b2199fbc8009a1486b89f987bdf9ba71ecd7083d6d",
        ),
        (
            "tinyflows-module-0.9.0-ubuntu-24.04-x86_64.tar.gz",
            "9a280dc919bb3aa5631808d958aaf70fd12d0dc00cacad01f889262577cef8cc",
        ),
        (
            "tinyflows-module-0.9.0-ubuntu-24.04-arm64.tar.gz",
            "833fc554c39ab6f01597cb73a6d859464bc81dfa94b3118a5cd58bbeb6484aca",
        ),
        (
            "tinyflows-module-0.9.0-ubuntu-22.04-x86_64.tar.gz",
            "77e8deb1197ecb58870e1db85c5c94d60ae9da77649c95face27a4a3dde4b146",
        ),
        (
            "tinyflows-module-0.9.0-ubuntu-22.04-arm64.tar.gz",
            "67f7df072b4c25393308f06078a4f9fc7fbb0111a282d7cb26bd364d702d1d4d",
        ),
        (
            "tinyflows-module-0.9.0-windows-2025-x86_64.zip",
            "28caaea85c38f81b7020b05c6f2ac1bd3d2d01d1be0adab7343b939103b35359",
        ),
        (
            "tinyflows-module-0.9.0-windows-2022-x86_64.zip",
            "8226887661e0dcf1a78858a61771f90a4256b8be3a302d5571544f97f5238ee7",
        ),
        (
            "tinyflows-module-0.9.0-windows-11-arm64.zip",
            "1daec664a8c6f92c13ece028c1c5311d9ef2471f12f4eedb2898d1f6ef709c6c",
        ),
    ];
    for (asset, (archive, sha256)) in record.assets.iter().zip(expected) {
        assert_eq!(asset.archive, archive);
        assert_eq!(asset.sha256, sha256, "{}", asset.archive);
    }
}
