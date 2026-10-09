use super::*;

#[test]
fn parse_embedded_returns_the_parsed_value() {
    let value = parse_embedded("test", r#"{"type":"object","properties":{}}"#);
    assert_eq!(value["type"], "object");
}

#[test]
#[should_panic(expected = "not valid JSON")]
fn parse_embedded_panics_with_site_on_malformed_json() {
    parse_embedded("test:1", "{ nope");
}

#[test]
fn static_schema_clones_independently() {
    let mut first = static_schema!(r#"{"a":1}"#);
    first["a"] = serde_json::json!(2);
    let second = static_schema!(r#"{"a":1}"#);
    assert_eq!(second["a"], 1);
}
