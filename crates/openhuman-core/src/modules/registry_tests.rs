use super::{find, ALL};
use tinybus::module::platform::candidates_for;

#[path = "registry_computer_release_tests.rs"]
mod computer_release_tests;

#[path = "registry_release_manifest_tests.rs"]
mod release_manifest_tests;

#[test]
#[cfg(feature = "web3")]
fn tinywallet_registry_matches_the_verified_v0260_release_manifest() {
    let wallet = find("tinywallet").expect("compiled TinyWallet module");
    assert_eq!(wallet.version, "0.26.0");
    #[cfg(feature = "web3")]
    assert_eq!(tinywallet_bus::CONTRACT_VERSION, (1, 14));
    assert_eq!(
        wallet.release_url,
        "https://github.com/tinyhumansai/tinywallet/releases/tag/v0.26.0"
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
            "tinywallet-module-0.26.0-archlinux-arm64.tar.gz",
            "b59ec08e44abf5079a19978b8adc3f6f2514a2bd8925c20429d554fe610a4c7e",
        ),
        (
            "archlinux-x86_64",
            "tinywallet-module-0.26.0-archlinux-x86_64.tar.gz",
            "b15f03b5c6ed054c79cdf6518c1c6674cfced37556f52d189d30ad86814090e5",
        ),
        (
            "fedora-43-arm64",
            "tinywallet-module-0.26.0-fedora-43-arm64.tar.gz",
            "f1be27c80bcc0cf93846ab12b9ba1151d17752875cb6603c23da1ee63d9b8a95",
        ),
        (
            "fedora-43-x86_64",
            "tinywallet-module-0.26.0-fedora-43-x86_64.tar.gz",
            "f22abcef72703f4ad3594f33eff9c16d3aa9850aefe2457635fab9f42b7add9f",
        ),
        (
            "fedora-44-arm64",
            "tinywallet-module-0.26.0-fedora-44-arm64.tar.gz",
            "39a5517ee0be1b3e8614f8691c5deb0b51817752007bd74dd781d877dae8313b",
        ),
        (
            "fedora-44-x86_64",
            "tinywallet-module-0.26.0-fedora-44-x86_64.tar.gz",
            "a48390bf558d1b14147790d880b792cf5f0e1aea7c79d38217f7d7ea53a2bc9d",
        ),
        (
            "macos-15-arm64",
            "tinywallet-module-0.26.0-macos-15-arm64.tar.gz",
            "04bc74a5187ac62b7416d88f847f9aa4b49914b8da864f82a19305fb43ace6f7",
        ),
        (
            "macos-15-x86_64",
            "tinywallet-module-0.26.0-macos-15-x86_64.tar.gz",
            "526119531f5132973c620ebceaf21e237cc39306a4638a31e88239b0816a6122",
        ),
        (
            "macos-26-arm64",
            "tinywallet-module-0.26.0-macos-26-arm64.tar.gz",
            "ad070c81c067578516e76c56edf68256ebb063826c98bfbcce2db16f2ab3ad0c",
        ),
        (
            "macos-26-x86_64",
            "tinywallet-module-0.26.0-macos-26-x86_64.tar.gz",
            "6c784834d21b3b0a94623f84dd3a132efd499788db8ae46844d7e3b42c0b4512",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinywallet-module-0.26.0-ubuntu-22.04-arm64.tar.gz",
            "bd404a0beecf749f6293685d3799abdf1ab701ec8b4fe6625bc7a14d9c890b79",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinywallet-module-0.26.0-ubuntu-22.04-x86_64.tar.gz",
            "87b7c1f9a76db09ab0dcbda1b02caef22e355c5c731bba7b53f8571aaa6a19fb",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinywallet-module-0.26.0-ubuntu-24.04-arm64.tar.gz",
            "9752e813ef204ae59759b8fee54cafe2b38b8b120dfe0ec451e28a7d20b9fdef",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinywallet-module-0.26.0-ubuntu-24.04-x86_64.tar.gz",
            "90bf726fb55fc0a6cd47e2e8a838d2e2ee7bf5217de19450a88479a089b85a17",
        ),
        (
            "windows-11-arm64",
            "tinywallet-module-0.26.0-windows-11-arm64.zip",
            "6eb5c829b6a38ae6732475b19d0bb7c106dc490ba55a0365208976c96eb275a2",
        ),
        (
            "windows-2022-x86_64",
            "tinywallet-module-0.26.0-windows-2022-x86_64.zip",
            "ca46336d7cad9d9728f0a7de58d69d2755bda23eeed617481c543102a1fb99f4",
        ),
        (
            "windows-2025-x86_64",
            "tinywallet-module-0.26.0-windows-2025-x86_64.zip",
            "c66b49e30adafe29033823b823785a68121ca6177b12a9ec1b989f1c68f2bde4",
        ),
    ];
    actual.sort_unstable();
    published.sort_unstable();
    assert_eq!(actual, published);
}

#[test]
fn tinybox_registry_matches_the_published_v021_release_manifest() {
    let record = find("tinybox").expect("compiled TinyBox module");
    assert_eq!(record.version, "0.2.1");
    assert_eq!(tinybox_bus::CONTRACT_VERSION, (2, 0));
    assert_eq!(
        record.release_url,
        "https://github.com/tinyhumansai/tinybox/releases/tag/v0.2.1"
    );

    let actual = record
        .assets
        .iter()
        .map(|asset| (asset.host_key, asset.archive, asset.sha256))
        .collect::<Vec<_>>();
    let published = [
        (
            "archlinux-arm64",
            "tinybox-0.2.1-archlinux-arm64.tar.gz",
            "ff2404f24840632a00d8107d02ea4ac258cc56b77e650653b6c440e769da282a",
        ),
        (
            "archlinux-x86_64",
            "tinybox-0.2.1-archlinux-x86_64.tar.gz",
            "3b8f14a175fb0e73578047192f5aeefe3f76014d5a1baf3fb85d63f300ee48fc",
        ),
        (
            "fedora-43-arm64",
            "tinybox-0.2.1-fedora-43-arm64.tar.gz",
            "8b53c7f60cfddfadda3f0269aee15676069d727c1fc8c10f7aa8211baa55292a",
        ),
        (
            "fedora-43-x86_64",
            "tinybox-0.2.1-fedora-43-x86_64.tar.gz",
            "aaafcd8b33a844e44362f7566d4c384c6a541abfd5888b0e415158c8d5bc0ea4",
        ),
        (
            "fedora-44-arm64",
            "tinybox-0.2.1-fedora-44-arm64.tar.gz",
            "5441fda12064b029e745229e0c9fd2fd64dcc8e30f3416f5cdbe877583524295",
        ),
        (
            "fedora-44-x86_64",
            "tinybox-0.2.1-fedora-44-x86_64.tar.gz",
            "1d48b6b734bba68dc3160b85f0cb9e8f7e96e36acab18afd30f71a20e002df2b",
        ),
        (
            "macos-15-arm64",
            "tinybox-0.2.1-macos-15-arm64.tar.gz",
            "8d59ecb80220913770a25ab32116c6ff8e55cdb6fac1db634eab3b8767e22690",
        ),
        (
            "macos-15-x86_64",
            "tinybox-0.2.1-macos-15-x86_64.tar.gz",
            "4c1f22592913ca5c57a840cca65a638493cb76b95cee4d88b7367d762a5afa01",
        ),
        (
            "macos-26-arm64",
            "tinybox-0.2.1-macos-26-arm64.tar.gz",
            "daaec8cbb2a8060783c53621794ae6c03b62c9f5295753c5040fe0bbdf75b98e",
        ),
        (
            "macos-26-x86_64",
            "tinybox-0.2.1-macos-26-x86_64.tar.gz",
            "de5c07812e54b5ae421bd28a0071566c36d124c07f87dde2eef6e0df83745350",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinybox-0.2.1-ubuntu-22.04-arm64.tar.gz",
            "488082b3619698a82dfe14d13547ffed90aabc9278f64384f39e7b2f3b8ec020",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinybox-0.2.1-ubuntu-22.04-x86_64.tar.gz",
            "d6dc0320b2944ebc3d1972d80d4a47e459c5ebb24d0c41ef0f64e5f442f90685",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinybox-0.2.1-ubuntu-24.04-arm64.tar.gz",
            "ff1f9e0b9658c463a4b8f1e6aeb36cc7f9bc8b97117e8d17fd8986245a8e5b97",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinybox-0.2.1-ubuntu-24.04-x86_64.tar.gz",
            "87f9017e0384babe52a43cc9002a23d13558cdbab2d1807caf6d48152684871a",
        ),
        (
            "windows-11-arm64",
            "tinybox-0.2.1-windows-11-arm64.zip",
            "86ab6d28269b9e7eb59b54f6534fdc3e9bff67ebfcf66311a0ce6fdd16eaf1d1",
        ),
        (
            "windows-2022-x86_64",
            "tinybox-0.2.1-windows-2022-x86_64.zip",
            "68b7089dd4141434d3dfc91b32fae688cae549a297d0a933797a3b2b43e92795",
        ),
        (
            "windows-2025-x86_64",
            "tinybox-0.2.1-windows-2025-x86_64.zip",
            "059e7fbc993627d866c6b57d59286fe80eefc4ebe6542d4535d0e0f7d08e22a9",
        ),
    ];

    assert_eq!(actual.len(), 17);
    assert_eq!(actual, published);
}
#[test]
fn tinymcp_registry_matches_the_published_v0202_release_manifest() {
    let record = find("tinymcp").expect("compiled TinyMCP module");
    assert_eq!(record.version, "0.20.2");
    assert_eq!(tinymcp_bus::CONTRACT_VERSION, (1, 28));
    assert_eq!(
        record.release_url,
        "https://github.com/tinyhumansai/tinymcp/releases/tag/v0.20.2"
    );
    let actual = record
        .assets
        .iter()
        .map(|asset| (asset.host_key, asset.archive, asset.sha256))
        .collect::<Vec<_>>();
    let published = [
        (
            "archlinux-rolling-arm64",
            "tinymcp-0.20.2-archlinux-arm64.tar.gz",
            "130292bd749ce4e075642966e827494dc3f645cd310bfd993579b65645d757eb",
        ),
        (
            "archlinux-rolling-x86_64",
            "tinymcp-0.20.2-archlinux-x86_64.tar.gz",
            "2395b184ef39b614914c4cf9a928733617a5b441c6de78e461dcc6c6989f21c4",
        ),
        (
            "fedora-43-arm64",
            "tinymcp-0.20.2-fedora-43-arm64.tar.gz",
            "1e3c747a255de2503476f600dfd18598f4f857c74998f96c97fe8e3ae2f3eff5",
        ),
        (
            "fedora-43-x86_64",
            "tinymcp-0.20.2-fedora-43-x86_64.tar.gz",
            "8e295d0576100535dd4feebdbceb6114fce6670656ee3f965f26ca20ef6347c3",
        ),
        (
            "fedora-44-arm64",
            "tinymcp-0.20.2-fedora-44-arm64.tar.gz",
            "86948bdb4d2a9928bc972b4490af5ab289dc7bca33c7198c9df674ccce966be3",
        ),
        (
            "fedora-44-x86_64",
            "tinymcp-0.20.2-fedora-44-x86_64.tar.gz",
            "2652a53066843e4a7d379bc5584e95566f2fa80dd29c9c916408022ca37b5bd0",
        ),
        (
            "macos-15-arm64",
            "tinymcp-0.20.2-macos-15-arm64.tar.gz",
            "369c875740741a7f60da6a29e55e6aad97b65a067190c90aba89b68401721a04",
        ),
        (
            "macos-15-x86_64",
            "tinymcp-0.20.2-macos-15-x86_64.tar.gz",
            "0cd9af717a68851e1030656558664a48bc68cf75a2bd1e4cf6714195cf295f76",
        ),
        (
            "macos-26-arm64",
            "tinymcp-0.20.2-macos-26-arm64.tar.gz",
            "fe69bcfae6634a6d8af67b6496f7f53eaa9361f14ee1706f0a0176be6c550670",
        ),
        (
            "macos-26-x86_64",
            "tinymcp-0.20.2-macos-26-x86_64.tar.gz",
            "3303a4bd0a40d1400f44691adf8c412c64999f804913839643c29589664e4ded",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinymcp-0.20.2-ubuntu-22.04-arm64.tar.gz",
            "cc96d0dbaffdb33de1727f17298ea7e7e9fb53a63199732554340133c5920b2d",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinymcp-0.20.2-ubuntu-22.04-x86_64.tar.gz",
            "f580249fe9f4b9e2587910e686a3bb0999c63ff461aaed9bd812ce7738a14a75",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinymcp-0.20.2-ubuntu-24.04-arm64.tar.gz",
            "386c0cd318da9e5c88c91fba21cfacbf2b5096768ce3fe609930eca73ee8c5d3",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinymcp-0.20.2-ubuntu-24.04-x86_64.tar.gz",
            "63b9bf334f3c1e0dbf7db795e33c18ab05849da72c83837bc58ac93b81e5668f",
        ),
        (
            "windows-11-arm64",
            "tinymcp-0.20.2-windows-11-arm64.zip",
            "bbd2c09d0cc4b8acd7af8ba985151d83c87c8b454748f35db44d22027099fa84",
        ),
        (
            "windows-2022-x86_64",
            "tinymcp-0.20.2-windows-2022-x86_64.zip",
            "51e5c3a435204913692fe006ed6da0e425a122536f9e2f888ba1e9f1d5400da0",
        ),
        (
            "windows-2025-x86_64",
            "tinymcp-0.20.2-windows-2025-x86_64.zip",
            "e7611602514858e6c3e9bbf942adcd4ea3f9d16d3fcfea10444a5a21b1a77eda",
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
    assert_eq!(search.version, "0.5.3");
    assert_eq!(tinysearch_bus::CONTRACT_VERSION, (2, 2));
    assert_eq!(
        search.asset_for("ubuntu-24.04-x86_64").unwrap().sha256,
        "78b5632d9c207f9b78ee83627bf35ae73172d0fb1fb7ea47273058f4c1f4c0b1"
    );
}
