use super::{find, ALL};
use tinybus::module::platform::candidates_for;

#[test]
fn tinycomputer_registry_matches_bus_contract_and_published_release() {
    let desktop = find("tinycomputer").expect("compiled computer module");
    assert_eq!(desktop.bus_name, tinycomputer_bus::names::INTERFACE);
    assert_eq!(desktop.object_path, tinycomputer_bus::names::OBJECT_PATH);
    assert_eq!(desktop.version, "0.10.0");
    assert_eq!(desktop.assets.len(), 7);
    assert_eq!(
        desktop.asset_for("macos-26-arm64").unwrap().sha256,
        "3f774d47841c9da0d4603072baa70089dba675f54a6fff9213874737e034ae9b"
    );
}

#[test]
fn tinybox_registry_matches_the_published_v015_release_manifest() {
    let record = find("tinybox").expect("compiled TinyBox module");
    assert_eq!(record.version, "0.1.15");
    assert_eq!(
        record.release_url,
        "https://github.com/tinyhumansai/tinybox/releases/tag/v0.1.15"
    );

    let actual = record
        .assets
        .iter()
        .map(|asset| (asset.host_key, asset.archive, asset.sha256))
        .collect::<Vec<_>>();
    let published = [
        (
            "ubuntu-24.04-x86_64",
            "tinybox-0.1.15-ubuntu-24.04-x86_64.tar.gz",
            "43cd2cc0e9e5733d206be45d4c28e5c007862c48b5a3942a32d95e3995ba19cf",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinybox-0.1.15-ubuntu-24.04-arm64.tar.gz",
            "9cafb08666bb479ce42adc37e003fb9dbca8f9b9b8d36880ec8dd3c67afa1f10",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinybox-0.1.15-ubuntu-22.04-x86_64.tar.gz",
            "4b55df76c83a626959d6c130b9c263a8d0c062a789c6566e1d920d9230dd3209",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinybox-0.1.15-ubuntu-22.04-arm64.tar.gz",
            "81a4e6e56434729ce924ef94e66d5200f1cb66ccbd512f40f1bfbed749595682",
        ),
        (
            "macos-26-arm64",
            "tinybox-0.1.15-macos-26-arm64.tar.gz",
            "9433c307e7e5ee827b04d31dec771b8f28012ed6086d03abccca43deb99ecd50",
        ),
        (
            "macos-26-x86_64",
            "tinybox-0.1.15-macos-26-x86_64.tar.gz",
            "18309b6692229b8abbecbfcb938d3e2cd509a58b067d44807355be40d48454a2",
        ),
        (
            "macos-15-arm64",
            "tinybox-0.1.15-macos-15-arm64.tar.gz",
            "fcbb3e7339c93fc8ee939ecc7fd38f1adf37c29ba322d4085705ae82afd049dc",
        ),
        (
            "macos-15-x86_64",
            "tinybox-0.1.15-macos-15-x86_64.tar.gz",
            "314063db104b0353974c56232b28e63a8da57f438a72776d079145ceef1a0ea1",
        ),
        (
            "windows-2025-x86_64",
            "tinybox-0.1.15-windows-2025-x86_64.zip",
            "0acd1af4e5f19c75e2ef3b914d08717a0c1513a571b82bf5f48e32267f99fb31",
        ),
        (
            "windows-2022-x86_64",
            "tinybox-0.1.15-windows-2022-x86_64.zip",
            "d36da0d16217e3415c5a60950f5fa555c81f5e0ebd0552022d1a85ad48a3a2b1",
        ),
        (
            "windows-11-arm64",
            "tinybox-0.1.15-windows-11-arm64.zip",
            "d5ec94330ffc439451a05ebf5f2cd603236df5003993d34788deb4ac5fe22f49",
        ),
    ];

    assert_eq!(actual, published);
}

#[test]
fn ids_and_bus_names_are_unique() {
    // Two records claiming one bus name is a conflict tinybus would only
    // surface at load time, on whichever one happened to be second.
    for (i, record) in ALL.iter().enumerate() {
        for other in &ALL[i + 1..] {
            assert_ne!(record.id, other.id, "duplicate module id");
            assert_ne!(record.bus_name, other.bus_name, "duplicate bus name");
        }
    }
}

#[test]
fn every_object_path_matches_its_bus_name() {
    // tinybus derives a module's object path from its bus name by replacing
    // dots with slashes, and admission compares the two. A mismatch here is
    // a module that downloads and then refuses to load.
    for record in ALL {
        assert_eq!(
            record.object_path,
            format!("/{}", record.bus_name.replace('.', "/")),
            "{} object path does not match its bus name",
            record.id
        );
    }
}

#[test]
fn every_digest_is_a_lowercase_sha256() {
    // An uppercase or truncated digest is refused by tinybus at download
    // time, which is a slow way to find a typo in this file.
    for record in ALL {
        for asset in record.assets {
            assert_eq!(
                asset.sha256.len(),
                64,
                "{} / {} digest is not 64 characters",
                record.id,
                asset.host_key
            );
            assert!(
                asset
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "{} / {} digest is not lowercase hex",
                record.id,
                asset.host_key
            );
        }
    }
}

#[test]
fn every_asset_name_carries_its_host_key_and_a_known_extension() {
    // tinybus selects the asset by exact name and requires a `.tar.gz` or
    // `.zip` archive, so a name that does not match its key is a module that
    // loads the wrong platform's library.
    for record in ALL {
        for asset in record.assets {
            assert!(
                asset.archive.contains(asset.host_key),
                "{} asset {} does not name its host key {}",
                record.id,
                asset.archive,
                asset.host_key
            );
            let windows = asset.host_key.starts_with("windows");
            assert_eq!(
                windows,
                asset.archive.ends_with(".zip"),
                "{} asset {} has the wrong archive format for its host",
                record.id,
                asset.archive
            );
            if !windows {
                assert!(asset.archive.ends_with(".tar.gz"));
            }
        }
    }
}

#[test]
fn every_asset_name_carries_the_pinned_version() {
    // The digests and the version have to describe one release; an asset
    // left behind at an older version would download bytes the digest
    // beside it never matched.
    for record in ALL {
        for asset in record.assets {
            assert!(
                asset.archive.contains(record.version),
                "{} asset {} is not from version {}",
                record.id,
                asset.archive,
                record.version
            );
        }
    }
}

#[test]
fn the_release_url_is_a_tag_on_github() {
    // tinybus refuses a URL that is not a tag, because a branch URL names
    // bytes that can change under a digest that was checked once.
    for record in ALL.iter().filter(|record| !record.assets.is_empty()) {
        assert!(
            record
                .release_url
                .starts_with("https://github.com/tinyhumansai/"),
            "{} release url is not an upstream GitHub URL",
            record.id
        );
        assert!(
            record.release_url.contains("/releases/tag/"),
            "{} release url is not a tag",
            record.id
        );
        assert!(
            record.release_url.ends_with(record.version),
            "{} release url does not name version {}",
            record.id,
            record.version
        );
    }
}

/// Every host key `platform` can produce, across the supported triples.
fn every_host_key() -> Vec<String> {
    let hosts = [
        ("linux", "x86_64", Some((2, 39))),
        ("linux", "aarch64", Some((2, 39))),
        ("linux", "x86_64", Some((2, 35))),
        ("linux", "aarch64", Some((2, 35))),
        ("macos", "x86_64", None),
        ("macos", "aarch64", None),
        ("windows", "x86_64", None),
        ("windows", "aarch64", None),
    ];
    let mut keys: Vec<String> = hosts
        .into_iter()
        .flat_map(|(os, arch, glibc)| candidates_for(os, arch, glibc))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

fn supported_host_keys(record: &super::ModuleRecord) -> Vec<String> {
    every_host_key()
        .into_iter()
        .filter(|key| record.id != "tinycomputer" || !key.starts_with("ubuntu-"))
        .collect()
}

#[test]
fn a_record_that_pins_a_release_covers_every_host_the_platform_table_offers() {
    // The two tables are written independently and would drift silently:
    // `platform` offering a key no release publishes turns a supported host
    // into an "unsupported host" at first use.
    //
    // Scoped to records that pin a release at all. A record with no assets
    // is a module this build knows but has no published artifact for; it
    // loads from a developer build or the module search path, and asserting
    // release coverage for a release that does not exist would only assert
    // that it does not exist. The partial-coverage case — the one that is
    // actually a bug — is caught below.
    for record in ALL.iter().filter(|record| !record.assets.is_empty()) {
        for key in supported_host_keys(record) {
            assert!(
                record.asset_for(&key).is_some(),
                "{} publishes no asset for {key}, which the platform table would ask for",
                record.id
            );
        }
    }
}

#[test]
fn a_record_publishes_for_every_host_or_for_none() {
    // Partial coverage is the drift that hurts: it looks supported until a
    // user on the missing platform reaches the feature. All-or-nothing keeps
    // "not published yet" distinguishable from "published and incomplete".
    for record in ALL {
        let host_keys = supported_host_keys(record);
        let covered = host_keys
            .into_iter()
            .filter(|key| record.asset_for(key).is_some())
            .count();
        assert!(
            covered == 0 || covered == supported_host_keys(record).len(),
            "{} publishes assets for {covered} of {} host keys",
            record.id,
            supported_host_keys(record).len()
        );
    }
}

#[test]
fn find_resolves_known_ids_only() {
    assert!(find("tinydocs").is_some());
    assert!(find("not-a-module").is_none());
}
