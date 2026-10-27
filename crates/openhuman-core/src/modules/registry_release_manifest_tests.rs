use super::*;

#[cfg(feature = "security-module")]
#[test]
fn tinysecurity_registry_matches_the_verified_v0182_release_manifest() {
    let security = find("tinysecurity").expect("compiled TinySecurity module");
    assert_eq!(security.version, "0.18.2");
    assert_eq!(tinysecurity_bus::CONTRACT_VERSION, (1, 33));
    assert_eq!(
        security.release_url,
        "https://github.com/senamakel/tinysecurity/releases/tag/v0.18.2"
    );

    let mut actual = security
        .assets
        .iter()
        .map(|asset| (asset.host_key, asset.archive, asset.sha256))
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut published = [
        (
            "macos-15-arm64",
            "tinysecurity-module-0.18.2-macos-15-arm64.tar.gz",
            "7cd14bf7bbb11bceb4e09560a8f66c941e28541e94ac42320156ba9a7473952b",
        ),
        (
            "macos-15-x86_64",
            "tinysecurity-module-0.18.2-macos-15-x86_64.tar.gz",
            "7829b6e6b41a34d789973e8f94f828235c26ae201050de3645dfd6b53568941c",
        ),
        (
            "macos-26-arm64",
            "tinysecurity-module-0.18.2-macos-26-arm64.tar.gz",
            "bc765db3337e98c4f0c96ba6fd47576181c9783e833f9bdeccc745dcab276dc5",
        ),
        (
            "macos-26-x86_64",
            "tinysecurity-module-0.18.2-macos-26-x86_64.tar.gz",
            "10575824c14ea3503170420523ecdb6b3fa33cecb85f9ff3d36817666bb7833e",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinysecurity-module-0.18.2-ubuntu-22.04-arm64.tar.gz",
            "34c3ef7b6fde18d525ae27fe09716ad2775a91b3152663a1f72b88a0da15e325",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinysecurity-module-0.18.2-ubuntu-22.04-x86_64.tar.gz",
            "000e9a3a07a8388fee3a6b823615998959d5255e584526b978b653959fad8a30",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinysecurity-module-0.18.2-ubuntu-24.04-arm64.tar.gz",
            "a91db19a360840fd5f8a7c341ef4856fcdc25db98cc597b7acdd80ac075ab046",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinysecurity-module-0.18.2-ubuntu-24.04-x86_64.tar.gz",
            "4c8d90347c37293d5ea773245e0ce9dda4a18234117736757564beeffea5f5fa",
        ),
        (
            "windows-11-arm64",
            "tinysecurity-module-0.18.2-windows-11-arm64.zip",
            "dc6e7b53e7bf65c9adea251d0f1f5852a89f0588c782dbdb6ac0bab08a33c8ef",
        ),
        (
            "windows-2022-x86_64",
            "tinysecurity-module-0.18.2-windows-2022-x86_64.zip",
            "105ccd4d6fc2787cefaedba2dcc2f5e40129c0b4759e41095099322da5d6058b",
        ),
        (
            "windows-2025-x86_64",
            "tinysecurity-module-0.18.2-windows-2025-x86_64.zip",
            "2b41ef005c4d0756233451ca8667fdce492b73a85234bd7f4e2af06949130b70",
        ),
    ];
    published.sort_unstable();
    assert_eq!(actual, published);
}

#[test]
fn tinychannels_registry_matches_scoped_runtime_release() {
    let record = find("tinychannels").expect("compiled TinyChannels module");
    assert_eq!(record.version, "0.1.40");
    assert_eq!(tinychannels_bus::CONTRACT_VERSION, 22);
    assert_eq!(
        tinychannels_bus::module_relay::MODULE_OPERATION_VERSION,
        (1, 23)
    );
    let expected = [
        (
            "archlinux-arm64",
            "tinychannels-module-0.1.40-archlinux-arm64.tar.gz",
            "2643b5aefb51a882739640666e456022921304b1ff3cf935005803e1baeae2db",
        ),
        (
            "archlinux-x86_64",
            "tinychannels-module-0.1.40-archlinux-x86_64.tar.gz",
            "b65e7e2e99af174166ce18acce7b3e9948b03f93644ac34958a2dd1d45b9783f",
        ),
        (
            "fedora-43-arm64",
            "tinychannels-module-0.1.40-fedora-43-arm64.tar.gz",
            "06470acac32353419b9e2c8eed0b13244de9957bc6d7575932b8bf5989579780",
        ),
        (
            "fedora-43-x86_64",
            "tinychannels-module-0.1.40-fedora-43-x86_64.tar.gz",
            "5956996b8fdc829e530a5cf808a8f9561dba7f91aead25df4f091a6c5ead605f",
        ),
        (
            "fedora-44-arm64",
            "tinychannels-module-0.1.40-fedora-44-arm64.tar.gz",
            "e449125869811077cb020b6dbd9f5dffc25fbf1526b8ee2402f0fac0b2f53bbc",
        ),
        (
            "fedora-44-x86_64",
            "tinychannels-module-0.1.40-fedora-44-x86_64.tar.gz",
            "f5ab13f94fe850ec90a95f1d527ee9c55ab3fdeb427f3c1c2e390c35b300d76e",
        ),
        (
            "macos-15-arm64",
            "tinychannels-module-0.1.40-macos-15-arm64.tar.gz",
            "98d31e633601c365200c679f288b13f9d7aef2023ebb9853bcbad690ddbbe1a0",
        ),
        (
            "macos-15-x86_64",
            "tinychannels-module-0.1.40-macos-15-x86_64.tar.gz",
            "4ac740b410102b29ba469f37f450b3e03fac3ce31ccb9d3538c1633025a7bd4d",
        ),
        (
            "macos-26-arm64",
            "tinychannels-module-0.1.40-macos-26-arm64.tar.gz",
            "ed2aab10a198193e5bebb5957095acad664a5a66a02d6142563815df6c353b53",
        ),
        (
            "macos-26-x86_64",
            "tinychannels-module-0.1.40-macos-26-x86_64.tar.gz",
            "210a7904dd942629af46d7ad36d0b0312f3a1cf0aa238b667ca6309ebf86c8e2",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinychannels-module-0.1.40-ubuntu-22.04-arm64.tar.gz",
            "48d360aea7849bb93050dbb56f9c28de6848572632908b635fb77df2a65c3bb1",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinychannels-module-0.1.40-ubuntu-22.04-x86_64.tar.gz",
            "6026081ac8ceb7b91daf0101364c92f99c10c5072a270b91eaeed6acad5d1f90",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinychannels-module-0.1.40-ubuntu-24.04-arm64.tar.gz",
            "0b4ef6b3c9c2c81f1efdbc5cbe8713478ff4e2f76682625465ee8c7120dfb798",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinychannels-module-0.1.40-ubuntu-24.04-x86_64.tar.gz",
            "bec22856f0b5f36f8c48f893c53d83015961a596d4ad56a33714ca378c2b4c98",
        ),
        (
            "windows-11-arm64",
            "tinychannels-module-0.1.40-windows-11-arm64.zip",
            "5f463711be6f8c856e13f77ba5d376b1b4e5b9519551411ae7bfd94a196bd633",
        ),
        (
            "windows-2022-x86_64",
            "tinychannels-module-0.1.40-windows-2022-x86_64.zip",
            "5df4ce49573c7d1e03a93b50cf092bf62dc6b92f50bcfe1471aff50a844268ce",
        ),
        (
            "windows-2025-x86_64",
            "tinychannels-module-0.1.40-windows-2025-x86_64.zip",
            "e8dd10cc102a7889d7d9ba8f50416b2ee3cec810f203555bf40cd0f157765552",
        ),
    ];
    assert_eq!(record.assets.len(), expected.len());
    for (key, archive, sha256) in expected {
        let asset = record.asset_for(key).expect("supported platform asset");
        assert_eq!(asset.archive, archive);
        assert_eq!(asset.sha256, sha256);
    }
}

#[cfg(feature = "voice")]
#[test]
fn voice_capture_release_pins_summary_contract_and_every_platform_digest() {
    let record = find("tinyvoice").unwrap();
    assert_eq!(record.version, "0.4.0");
    assert_eq!(tinyvoice_bus::CONTRACT_VERSION, (1, 12));
    let expected = [
        (
            "archlinux-arm64",
            "tinyvoice-module-0.4.0-archlinux-arm64.tar.gz",
            "216a99dc13cb7216fd8682bb88c87e381edcc4432ac4eb865db2b473cf18656a",
        ),
        (
            "archlinux-x86_64",
            "tinyvoice-module-0.4.0-archlinux-x86_64.tar.gz",
            "1de1939573e21248ede86a568f511e8d1f4a493e6cd38f5fdcd942d5000734cc",
        ),
        (
            "fedora-43-arm64",
            "tinyvoice-module-0.4.0-fedora-43-arm64.tar.gz",
            "97164ab70529a8f38b7650c3bcbb92798c55140275f0302a811148f9a37a39e2",
        ),
        (
            "fedora-43-x86_64",
            "tinyvoice-module-0.4.0-fedora-43-x86_64.tar.gz",
            "031cad7e8a4e47a2641d7a56ff8c6ca83a05b74b6c3545063956a527acea7476",
        ),
        (
            "fedora-44-arm64",
            "tinyvoice-module-0.4.0-fedora-44-arm64.tar.gz",
            "8697a58237c102f48ac68a8ea8a5ea67675670b1fb495d6ecb00cca9051a882e",
        ),
        (
            "fedora-44-x86_64",
            "tinyvoice-module-0.4.0-fedora-44-x86_64.tar.gz",
            "2cc8feb4ee0662bf0dfc558640da63d45095efba119f11f9049b9e840e411bda",
        ),
        (
            "macos-15-arm64",
            "tinyvoice-module-0.4.0-macos-15-arm64.tar.gz",
            "de6907175ccde6cc2c25140d75220537dd44d986e5e3b5c51586bf8deead3d88",
        ),
        (
            "macos-15-x86_64",
            "tinyvoice-module-0.4.0-macos-15-x86_64.tar.gz",
            "5405bc4d4a08c62b33c6a1ae671a512bd5379304b5c969cf34a7138bc887dd08",
        ),
        (
            "macos-26-arm64",
            "tinyvoice-module-0.4.0-macos-26-arm64.tar.gz",
            "b08ffb8ab2883a7c35b245f48a38eb226b48271e7d862b95855a55cb82501685",
        ),
        (
            "macos-26-x86_64",
            "tinyvoice-module-0.4.0-macos-26-x86_64.tar.gz",
            "c0bd82d8164d603af8137c578c93e3b27f6018af8ddbffd93d103cf8878f3261",
        ),
        (
            "ubuntu-22.04-arm64",
            "tinyvoice-module-0.4.0-ubuntu-22.04-arm64.tar.gz",
            "42e2e1a10e195f062b47e593119171ebc8229c740fe3d2bb882d94aa944e6344",
        ),
        (
            "ubuntu-22.04-x86_64",
            "tinyvoice-module-0.4.0-ubuntu-22.04-x86_64.tar.gz",
            "0030adce6ffb5d2a196ab35bcb3c4edcc0ae205810c729bc2a04af58aa7a7c78",
        ),
        (
            "ubuntu-24.04-arm64",
            "tinyvoice-module-0.4.0-ubuntu-24.04-arm64.tar.gz",
            "a3ade2060ab22e16115a926e1c3fa32536777ec122565dfdc1f01a364c15979e",
        ),
        (
            "ubuntu-24.04-x86_64",
            "tinyvoice-module-0.4.0-ubuntu-24.04-x86_64.tar.gz",
            "4ddc8eebce5d644eff123488cd4422b9e227915f3d8793e0a838f141ca7a04e9",
        ),
        (
            "windows-11-arm64",
            "tinyvoice-module-0.4.0-windows-11-arm64.zip",
            "e44c9457dfde9aacbda869954201237f1e4ddba97ddcf6c79c3de92e04b7b171",
        ),
        (
            "windows-2022-x86_64",
            "tinyvoice-module-0.4.0-windows-2022-x86_64.zip",
            "82a8f02df915a532cf4115f8ed2ffa290ec0690e350d003bf30f98b8c07af02a",
        ),
        (
            "windows-2025-x86_64",
            "tinyvoice-module-0.4.0-windows-2025-x86_64.zip",
            "c220042612de4f8c728907d5f5ff44264a97bf15943ac91758434a2c0132b8d0",
        ),
    ];
    assert_eq!(record.assets.len(), expected.len());
    for (key, archive, digest) in expected {
        let asset = record.asset_for(key).unwrap();
        assert_eq!(asset.archive, archive);
        assert_eq!(asset.sha256, digest);
    }
}
