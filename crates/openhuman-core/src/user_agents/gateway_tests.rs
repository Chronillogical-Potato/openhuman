use super::*;

const SECRET: &str = "gateway-secret-0123456789abcdef0123456789";

#[test]
fn a_fresh_signature_verifies() {
    let header = sign(SECRET, "alice", 1_000);
    assert!(header.starts_with("t=1000,v1="));
    verify(SECRET, "alice", &header, 1_000).unwrap();
    verify(SECRET, "alice", &header, 1_000 + SIGNATURE_WINDOW_SECS).unwrap();
    verify(SECRET, "alice", &header, 1_000 - SIGNATURE_WINDOW_SECS).unwrap();
}

#[test]
fn a_signature_is_bound_to_the_user_the_secret_and_the_window() {
    let header = sign(SECRET, "alice", 1_000);
    assert!(verify(SECRET, "bob", &header, 1_000).is_err(), "another user");
    assert!(verify("other-secret", "alice", &header, 1_000).is_err(), "another secret");
    let stale = verify(SECRET, "alice", &header, 1_000 + SIGNATURE_WINDOW_SECS + 1);
    assert!(stale.unwrap_err().contains("window"));
}

#[test]
fn malformed_signatures_are_refused() {
    for header in ["", "v1=00", "t=1000", "t=abc,v1=00", "t=1000,v1=zz"] {
        assert!(verify(SECRET, "alice", header, 1_000).is_err(), "{header:?}");
    }
}

#[test]
fn a_tampered_tag_is_refused() {
    let header = sign(SECRET, "alice", 1_000);
    let mut tampered = header.clone();
    let last = tampered.pop().unwrap();
    tampered.push(if last == '0' { '1' } else { '0' });
    assert!(verify(SECRET, "alice", &tampered, 1_000).unwrap_err().contains("does not match"));
}

#[test]
fn no_user_header_is_the_operator_plane() {
    assert!(matches!(
        resolve_scope(None, None, SECRET, 0),
        Ok(GatewayScope::Operator)
    ));
}
