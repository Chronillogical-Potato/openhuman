use super::{find, ALL};
use tinybus::module::platform::candidates_for;

#[path = "registry_computer_release_tests.rs"]
mod computer_release_tests;

#[path = "registry_release_manifest_tests.rs"]
mod release_manifest_tests;

#[test]
#[cfg(feature = "web3")]
fn tinywallet_registry_matches_the_verified_v0270_release_manifest() {
    let wallet = find("tinywallet").expect("compiled TinyWallet module");
    assert_eq!(wallet.version, "0.27.0");
    #[cfg(feature = "web3")]
    assert_eq!(tinywallet_bus::CONTRACT_VERSION, (1, 14));
    assert_eq!(
        wallet.release_url,
        "https://github.com/tinyhumansai/tinywallet/releases/tag/v0.27.0"
    );

    let mut actual = wallet
        .assets
        .iter()
        .map(|asset| (asset.host_key, asset.archive, asset.sha256))
        .collect::<Vec<_>>();
    assert_eq!(wallet.assets.len(), 17);
    let mut published = [
        (
            "archlinux-arm64",
            "tinywallet-module-0.27.0-archlinux-arm64.tar.gz",
            "739b73d4fe246b108d3d8c15b96e358219e3357554663054bde9b4cfcc1e1547",
        ),
        (
            "archlinux-x86_64",
            "tinywallet-module-0.27.0-archlinux-x86_64.tar.gz",
            "30ad7f2de3d10d584d677412cd1fa6fc124083fb28ad5c58fcdb63a8c3d88f79",
        ),
        (
            "fedora-43-arm64",
            "tinywallet-module-0.27.0-fedora-43-arm64.tar.gz",
            "e2a185ff78c0f636a1ccbc7a5d6d91e23121394fa5d64683058f33e24b414771",
        ),
        (
            "fedora-43-x86_64",
            "tinywallet-module-0.27.0-fedora-43-x86_64.tar.gz",
            "28f03bfde6eb63dde25e0f6fafaf2ce25e453afdf56a5c9a3277d47bf2fe5400",
        ),
        (
            "fedora-44-arm64",
            "tinywallet-module-0.27.0-fedora-44-arm64.tar.gz",
            "574cf3773f405e282c3d42f4fb8ba8e3037e7abcd06ee63c719a134c100f9748",
        ),
        (
            "fedora-44-x86_64",
            "tinywallet-module-0.27.0-fedora-44-x86_64.tar.gz",
            "af05c0582364306a0bf885df059bea601044cfd40c40c7a14af043499053dc71",
        ),
        (
            "macos-15-arm64",
            "tinywallet-module-0.27.0-macos-15-arm64.tar.gz",
            "554375d3e45eb9332454323937c7923e34a75934a99b2185bff35503b7abeec5",
        ),
        (
            "macos-15-x86_64",
            "tinywallet-module-0.27.0-macos-15-x86_64.tar.gz",
            "ad4a098b29dedd350bd60bf4d0194fa89cc2f79bff47447f7d52b440f702730a",
        ),
        (
            "macos-26-arm64",
            "tinywallet-module-0.27.0-macos-26-arm64.tar.gz",
            "762c4fa1a283df5d499610314a6d35b002663e00f4b57105d56047a5d0cb83a8",
        ),
        (
            "macos-26-x86_64",
            "tinywallet-module-0.27.0-macos-26-x86_64.tar.gz",
            "5973882752b659ac5ef9906fcc418e55796e3cc1375b3be141e16a624bc666c1",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinywallet-module-0.27.0-ubuntu-22.04-arm64.tar.gz",
            "927227ee8d8a23a12966ccb2de972fcab07e80fb6527dad840170e4ffd64f087",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinywallet-module-0.27.0-ubuntu-22.04-x86_64.tar.gz",
            "a031e81172452657a506cd769a41f8702c06d80a75b72ebf6dc9a0f4a8317c25",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinywallet-module-0.27.0-ubuntu-24.04-arm64.tar.gz",
            "6e055459e513707e58b4d07c836a3659a453c3753ee8c632e858930c150b66f4",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinywallet-module-0.27.0-ubuntu-24.04-x86_64.tar.gz",
            "cab0be708a52e8713df7cc57dc13717e3899ce3c8ec57819741b37dc463ccfc7",
        ),
        (
            "windows-11-arm64",
            "tinywallet-module-0.27.0-windows-11-arm64.zip",
            "9e458762f740e166b9b028336b6faed3762dfb71bf46eb57c71ad7e3ec2dc9d3",
        ),
        (
            "windows-2022-x86_64",
            "tinywallet-module-0.27.0-windows-2022-x86_64.zip",
            "ff389c8951c0721b31bb8eb4a71e86424f053923eab10464c8ee68bbb1ec25a1",
        ),
        (
            "windows-2025-x86_64",
            "tinywallet-module-0.27.0-windows-2025-x86_64.zip",
            "b01f58d66e73f73b510bf755fb89d1fa77a5caad00033405511ce12b84086632",
        ),
    ];
    actual.sort_unstable();
    published.sort_unstable();
    assert_eq!(actual, published);
}

#[test]
fn tinybox_registry_matches_the_published_v023_release_manifest() {
    let record = find("tinybox").expect("compiled TinyBox module");
    assert_eq!(record.version, "0.2.3");
    assert_eq!(tinybox_bus::CONTRACT_VERSION, (2, 1));
    assert_eq!(
        record.release_url,
        "https://github.com/tinyhumansai/tinybox/releases/tag/v0.2.3"
    );

    let actual = record
        .assets
        .iter()
        .map(|asset| (asset.host_key, asset.archive, asset.sha256))
        .collect::<Vec<_>>();
    let published = [
        (
            "archlinux-arm64",
            "tinybox-0.2.3-archlinux-arm64.tar.gz",
            "f5793f3163855faeff97f06ae3ee117c8ab3a6a28e166e0a4c10d663c2c98c59",
        ),
        (
            "archlinux-x86_64",
            "tinybox-0.2.3-archlinux-x86_64.tar.gz",
            "0c275b1eb643f01781eb03bac9efcfa9516c14e387aea23f4d740ef2c4a9f3fb",
        ),
        (
            "fedora-43-arm64",
            "tinybox-0.2.3-fedora-43-arm64.tar.gz",
            "a78cb0729a3f513fa32ecd3690781ffe76f256f9ab31c3008d72cc28fbab745a",
        ),
        (
            "fedora-43-x86_64",
            "tinybox-0.2.3-fedora-43-x86_64.tar.gz",
            "1df8a55ed71b2e39e7b6767df3f9d4c8bd6d096d97fad81aa4d24c2f4e200459",
        ),
        (
            "fedora-44-arm64",
            "tinybox-0.2.3-fedora-44-arm64.tar.gz",
            "5f5df28fda5f94d594f8f2be60d4a0b1734d525f0db2ffe7d725e20eeb26fdd8",
        ),
        (
            "fedora-44-x86_64",
            "tinybox-0.2.3-fedora-44-x86_64.tar.gz",
            "f30ec1e1b25f6e760361f5878b0852029e010710e4dfad50bee13e08ac6bf25b",
        ),
        (
            "macos-15-arm64",
            "tinybox-0.2.3-macos-15-arm64.tar.gz",
            "b23fba12090d384033f604a093213f1dbad6c211719c8ac1330245862010f38e",
        ),
        (
            "macos-15-x86_64",
            "tinybox-0.2.3-macos-15-x86_64.tar.gz",
            "e3e9178df6c1dea7c3f08eff9df06dcdf8d1e0f83ba23dfdafb70867d991af6a",
        ),
        (
            "macos-26-arm64",
            "tinybox-0.2.3-macos-26-arm64.tar.gz",
            "7e524cffcd7e2ad1ad21a393ca2b9ccdad2cfb4563b331270b182180d5f1c719",
        ),
        (
            "macos-26-x86_64",
            "tinybox-0.2.3-macos-26-x86_64.tar.gz",
            "e8bd5991e261db8f8560f94c119dfa987be690dc19f8b898dd60686d29ab79e4",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinybox-0.2.3-ubuntu-22.04-arm64.tar.gz",
            "3cd95331c40559de0c36b155ee596bdc99e057a32f0e54a4e5cfc652d6c60ab1",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinybox-0.2.3-ubuntu-22.04-x86_64.tar.gz",
            "aa03d3e8f88d635ea2ab134fdf27454326e0e43a696cd195ea3e626a1ed7aaf8",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinybox-0.2.3-ubuntu-24.04-arm64.tar.gz",
            "649309f8ed2b6739ae03dbd6b944bd820788c713f99b521a4ef65c762485a2eb",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinybox-0.2.3-ubuntu-24.04-x86_64.tar.gz",
            "8c3013d933bd9dc235975db5f5046da3c982511f7425502f4f4b288081b89376",
        ),
        (
            "windows-11-arm64",
            "tinybox-0.2.3-windows-11-arm64.zip",
            "bad2e51d65f6b37d8cb7464e3431caee640d11fae61d2ce786b570e6c31173e1",
        ),
        (
            "windows-2022-x86_64",
            "tinybox-0.2.3-windows-2022-x86_64.zip",
            "0f495036f98e88e566376b2c2180b1d09d597f433b3f9b34a88786c14414f61b",
        ),
        (
            "windows-2025-x86_64",
            "tinybox-0.2.3-windows-2025-x86_64.zip",
            "2119e8ba6d735aed78c0afb1d0ccb69416e048abff56ad8e46e4efd742c52ff8",
        ),
    ];

    assert_eq!(actual.len(), 17);
    assert_eq!(actual, published);
}
#[test]
fn tinymcp_registry_matches_the_published_v0204_release_manifest() {
    let record = find("tinymcp").expect("compiled TinyMCP module");
    assert_eq!(record.version, "0.20.4");
    assert_eq!(tinymcp_bus::CONTRACT_VERSION, (1, 28));
    assert_eq!(
        record.release_url,
        "https://github.com/tinyhumansai/tinymcp/releases/tag/v0.20.4"
    );
    let actual = record
        .assets
        .iter()
        .map(|asset| (asset.host_key, asset.archive, asset.sha256))
        .collect::<Vec<_>>();
    let published = [
        (
            "archlinux-rolling-arm64",
            "tinymcp-0.20.4-archlinux-arm64.tar.gz",
            "d3e414f18bfe29346e8d25ac3bdca0c36ed294f19e4c2eb335472ea2824e33e2",
        ),
        (
            "archlinux-rolling-x86_64",
            "tinymcp-0.20.4-archlinux-x86_64.tar.gz",
            "1a6d936e07665fdaa702c1b70f31386194e9288a9a91bf0dafa5e86dfa4c0c64",
        ),
        (
            "fedora-43-arm64",
            "tinymcp-0.20.4-fedora-43-arm64.tar.gz",
            "e96a0bf7100a23ee86636ecc2e04180b3001810d9102b24a29cb8a45f4fac295",
        ),
        (
            "fedora-43-x86_64",
            "tinymcp-0.20.4-fedora-43-x86_64.tar.gz",
            "67ba0b8a70b7d124bdb19c39b32fe312cbf26d3d724f4d63f1f32d4d81d1d06c",
        ),
        (
            "fedora-44-arm64",
            "tinymcp-0.20.4-fedora-44-arm64.tar.gz",
            "a10a30eac6b9bb0d7b0ab68fe6007e9bc3440572318d02082fc881158c441578",
        ),
        (
            "fedora-44-x86_64",
            "tinymcp-0.20.4-fedora-44-x86_64.tar.gz",
            "fd567d48f8f55b2953b4961a932da8d0357cc394c8115c24da7a4218f6206091",
        ),
        (
            "macos-15-arm64",
            "tinymcp-0.20.4-macos-15-arm64.tar.gz",
            "8f237c1fa91ecce4475505cbf4677c04059d1c4d16e8a63a6e60944f51afb41e",
        ),
        (
            "macos-15-x86_64",
            "tinymcp-0.20.4-macos-15-x86_64.tar.gz",
            "0ff41bc259ed0e95b83926f2917e0d76d3766e6bbd10c723e8bd42bce562e277",
        ),
        (
            "macos-26-arm64",
            "tinymcp-0.20.4-macos-26-arm64.tar.gz",
            "95e1294d191b4f8629b8bf525b83c2bc8ee3c7044dc8f6b77f5a4f44b5327c8c",
        ),
        (
            "macos-26-x86_64",
            "tinymcp-0.20.4-macos-26-x86_64.tar.gz",
            "c7d9b7291199dbeec97ed7fd97fbf5f08267040b2bbd27f37a3d03559e7bc1f0",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinymcp-0.20.4-ubuntu-22.04-arm64.tar.gz",
            "6243a3fe7b6c6a2972874380c057ec612c7477747e1a744cced6a559ed056301",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinymcp-0.20.4-ubuntu-22.04-x86_64.tar.gz",
            "f116c78d8dacceaae42935b34ff3e5fe5e537ab0c441a7750d15404547500210",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinymcp-0.20.4-ubuntu-24.04-arm64.tar.gz",
            "0ee8f3f4032a5480883c1c4beae651673858b4329f9014796a2e9f1f71c1a5ad",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinymcp-0.20.4-ubuntu-24.04-x86_64.tar.gz",
            "83ddf549809be714c82ac93c0ad393dc7c52474941c20fe7ae17f78a24f05822",
        ),
        (
            "windows-11-arm64",
            "tinymcp-0.20.4-windows-11-arm64.zip",
            "8773fafd0fa68360969c0d7cf58920e29f82ac4573c4371a546275fd8ba66a53",
        ),
        (
            "windows-2022-x86_64",
            "tinymcp-0.20.4-windows-2022-x86_64.zip",
            "7ecfb93c4ebb242726dba6203512745d7eec5c19671ef560e836f316e1e04aea",
        ),
        (
            "windows-2025-x86_64",
            "tinymcp-0.20.4-windows-2025-x86_64.zip",
            "4562fd55fa3e3126f455391e69b67e71736b5fbb73ac0eecf491da83d35507fc",
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
    // `.zip` archive. Most archive platform stems match their host keys;
    // TinyMCP's release uses the canonical `archlinux` stem for the rolling
    // Arch host keys.
    for record in ALL {
        for asset in record.assets {
            let archive_host_key = if record.id == "tinymcp" {
                asset
                    .host_key
                    .strip_prefix("archlinux-rolling-")
                    .map(|suffix| format!("archlinux-{suffix}"))
                    .unwrap_or_else(|| asset.host_key.to_owned())
            } else {
                asset.host_key.to_owned()
            };
            assert!(
                asset.archive.contains(&archive_host_key),
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
            record.release_url.starts_with("https://github.com/"),
            "{} release url is not a GitHub URL",
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

fn supported_host_keys(_record: &super::ModuleRecord) -> Vec<String> {
    every_host_key()
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
    assert!(find("tinymcp").is_some());
    assert!(find("not-a-module").is_none());
}

#[test]
fn every_record_names_a_source_manifest_that_exists_in_its_submodule() {
    let vendor = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor");
    for record in ALL {
        let spec = &record.source;
        assert!(!spec.package.is_empty(), "{}: package", record.id);
        assert!(!spec.lib_name.is_empty(), "{}: lib_name", record.id);
        assert_eq!(spec.submodule, record.id, "{}: submodule dir", record.id);
        let manifest = vendor.join(spec.submodule).join(spec.manifest);
        assert!(
            manifest.is_file(),
            "{}: source manifest {} is missing (is the submodule initialised?)",
            record.id,
            spec.manifest
        );
    }
}

#[test]
fn source_artifact_file_names_follow_the_platform_convention() {
    for record in ALL {
        let name = record.source.artifact_filename();
        assert!(name.contains(record.source.lib_name), "{}", record.id);
        if cfg!(target_os = "linux") {
            assert!(name.starts_with("lib") && name.ends_with(".so"), "{name}");
        }
    }
}

#[test]
fn tinysearch_registry_matches_bus_contract_and_published_release() {
    let search = find("tinysearch").expect("compiled search module");
    assert_eq!(search.bus_name, tinysearch_bus::names::INTERFACE);
    assert_eq!(search.object_path, tinysearch_bus::names::OBJECT_PATH);
    assert_eq!(search.version, "0.5.4");
    assert_eq!(tinysearch_bus::CONTRACT_VERSION, (2, 2));
    assert_eq!(
        search.asset_for("ubuntu-24.04-x86_64").unwrap().sha256,
        "44fc5fe514416063542253f3f3dd3ff5c68042eaba29de5777a2bf0a4b33f100"
    );
}
