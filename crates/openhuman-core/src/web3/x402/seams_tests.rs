use super::*;

use base64::Engine as _;
use reqwest::header::{HeaderMap, HeaderValue};
use tinywallet_bus::wire::Signature;
use tinywallet_x402::protocol::{PaymentBuilder, X402Error};
use tinywallet_x402::wire::{
    PaymentExtra, PaymentRequired, PaymentRequirements, ResourceInfo, SOLANA_MAINNET_CAIP2,
    USDC_MINT_MAINNET, X402_VERSION,
};

// ---------------------------------------------------------------------------
// Signature decoding: what the wallet module returns, as bytes the crate takes
// ---------------------------------------------------------------------------

#[test]
fn an_ed25519_signature_becomes_its_64_bytes() {
    let bytes = signature_bytes(Signature::Ed25519 {
        signature_hex: "ab".repeat(64),
    })
    .unwrap();
    assert_eq!(bytes, vec![0xab; 64]);
}

#[test]
fn a_secp256k1_signature_is_r_s_then_the_recovery_id() {
    let bytes = signature_bytes(Signature::Secp256k1 {
        rs_hex: "cd".repeat(64),
        recovery_id: 1,
    })
    .unwrap();
    assert_eq!(bytes.len(), 65);
    assert_eq!(&bytes[..64], &[0xcd; 64][..]);
    assert_eq!(bytes[64], 1);
}

#[test]
fn a_signature_that_is_not_hex_is_rejected() {
    let err = signature_bytes(Signature::Ed25519 {
        signature_hex: "zz".into(),
    })
    .unwrap_err();
    assert!(err.starts_with("invalid signature hex: "), "{err}");
    let err = signature_bytes(Signature::Secp256k1 {
        rs_hex: "zz".into(),
        recovery_id: 0,
    })
    .unwrap_err();
    assert!(err.starts_with("invalid signature hex: "), "{err}");
}

#[test]
fn a_secp256k1_signature_of_the_wrong_length_is_malformed() {
    let err = signature_bytes(Signature::Secp256k1 {
        rs_hex: "cd".repeat(63),
        recovery_id: 0,
    })
    .unwrap_err();
    assert_eq!(err, "the wallet module returned a malformed signature");
}

#[test]
fn payment_chains_map_onto_the_wallets_chains() {
    assert!(matches!(
        wallet_chain(PaymentChain::Solana),
        WalletChain::Solana
    ));
    assert!(matches!(wallet_chain(PaymentChain::Evm), WalletChain::Evm));
    assert_eq!(
        bus_chain(PaymentChain::Solana),
        tinywallet_bus::Chain::Solana
    );
    assert_eq!(bus_chain(PaymentChain::Evm), tinywallet_bus::Chain::Evm);
}

// ---------------------------------------------------------------------------
// The seams compose into the crate's payment flow
// ---------------------------------------------------------------------------

#[test]
fn the_proxy_policy_yields_a_buildable_client() {
    let builder = RuntimeProxyPolicy.apply(reqwest::Client::builder(), "tool.x402_request");
    assert!(builder.build().is_ok());
}

#[test]
fn the_tool_is_built_from_the_host_seams() {
    use tinytools::Tool;
    assert_eq!(request_tool().name(), "x402_request");
    assert_eq!(super::super::request_tool().name(), "x402_request");
}

fn solana_challenge_headers() -> HeaderMap {
    let challenge = PaymentRequired {
        x402_version: X402_VERSION,
        error: None,
        resource: ResourceInfo {
            url: "https://x402.example.test/thing".into(),
            description: None,
            mime_type: None,
        },
        accepts: vec![PaymentRequirements {
            scheme: "exact".into(),
            network: SOLANA_MAINNET_CAIP2.into(),
            amount: "10000".into(),
            asset: USDC_MINT_MAINNET.into(),
            pay_to: "2wKupLR9q6wXYppw8Gr2NvWxKBUqm4PPJKkQfoxHDBg4".into(),
            max_timeout_seconds: 60,
            extra: Some(PaymentExtra {
                fee_payer: Some("EwWqGE4ZFKLofuestmU4LDdK7XM1N4ALgdZccwYugwGd".into()),
                memo: None,
                name: None,
                version: None,
            }),
        }],
        extensions: serde_json::Map::new(),
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        "PAYMENT-REQUIRED",
        HeaderValue::from_str(
            &base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_vec(&challenge).unwrap()),
        )
        .unwrap(),
    );
    headers
}

/// With no wallet set up, the failure must surface from the *wallet seam* —
/// proving the facade reached the crate's budget check, the crate reached its
/// payment builder, and the builder reached the host signer — and read exactly
/// as it did before the flow moved into the crate.
#[tokio::test]
async fn a_payment_reaches_the_host_wallet_through_the_crate() {
    let _wallet_lock = crate::web3::wallet::test_support::TEST_LOCK.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let _workspace = crate::web3::wallet::test_support::set_workspace_env_for_test(&temp).await;
    super::super::init_ledger(temp.path(), "seam-test");

    let err =
        super::super::handle_402_and_pay(&solana_challenge_headers(), "https://x402.example.test")
            .await
            .unwrap_err();

    match err {
        X402Error::Wallet(message) => {
            assert!(message.starts_with("wallet secret: "), "{message}");
            assert!(
                message.contains(crate::web3::wallet::WALLET_NOT_CONFIGURED_MESSAGE),
                "{message}"
            );
        }
        other => panic!("expected a wallet error, got {other}"),
    }
}

#[tokio::test]
async fn the_signer_reports_a_missing_wallet_for_either_chain() {
    let _wallet_lock = crate::web3::wallet::test_support::TEST_LOCK.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let _workspace = crate::web3::wallet::test_support::set_workspace_env_for_test(&temp).await;

    for chain in [PaymentChain::Solana, PaymentChain::Evm] {
        let err = WalletPaymentSigner.account(chain).await.unwrap_err();
        assert!(err.starts_with("wallet secret: "), "{err}");
        let err = WalletPaymentSigner
            .sign(chain, b"message", SignScheme::Ed25519)
            .await
            .unwrap_err();
        assert!(err.starts_with("wallet secret: "), "{err}");
    }
}

#[tokio::test]
async fn the_payment_builder_is_object_safe_for_the_facade() {
    let _builder: Box<dyn PaymentBuilder> = Box::new(payments());
}

// ---------------------------------------------------------------------------
// The thread seam: which chat thread a payment belongs to
// ---------------------------------------------------------------------------

fn chat_ctx(thread: &str) -> crate::security::approval::ApprovalChatContext {
    crate::security::approval::ApprovalChatContext {
        thread_id: format!("thread-{thread}"),
        client_id: format!("client-{thread}"),
        request_id: None,
    }
}

/// The ledger records a payment's thread from this read. If it stopped reading
/// `APPROVAL_CHAT_CONTEXT`, every tool payment would silently lose its thread.
#[tokio::test]
async fn the_thread_scope_reads_the_approval_chat_context_task_local() {
    use tinywallet_x402::thread::ThreadScope;

    assert_eq!(
        TaskLocalThread.current_thread(),
        None,
        "outside a chat turn there is no thread"
    );

    let inside = crate::security::approval::APPROVAL_CHAT_CONTEXT
        .scope(chat_ctx("a"), async { TaskLocalThread.current_thread() })
        .await;
    assert_eq!(inside.as_deref(), Some("thread-a"));

    // Nested scopes answer with the innermost thread, and the outer one is
    // restored afterwards.
    let (inner, outer) = crate::security::approval::APPROVAL_CHAT_CONTEXT
        .scope(chat_ctx("outer"), async {
            let inner = crate::security::approval::APPROVAL_CHAT_CONTEXT
                .scope(chat_ctx("inner"), async {
                    TaskLocalThread.current_thread()
                })
                .await;
            (inner, TaskLocalThread.current_thread())
        })
        .await;
    assert_eq!(inner.as_deref(), Some("thread-inner"));
    assert_eq!(outer.as_deref(), Some("thread-outer"));
}
