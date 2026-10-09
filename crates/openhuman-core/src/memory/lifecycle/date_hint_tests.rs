use super::*;

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

#[test]
fn a_range_in_the_answer_is_a_hint_in_the_users_zone() {
    let hint = parse(
        "```json\n{\"from\":\"2026-10-06\",\"to\":\"2026-10-08\"}\n```",
        "Asia/Kolkata",
    )
    .expect("a hint");
    assert_eq!(
        hint,
        TimeHint::new(day(6), day(8), Some("Asia/Kolkata".into())).unwrap()
    );
    let single = parse("{\"from\":\"2026-10-03\"}", "UTC").expect("one day");
    assert_eq!((single.from, single.to), (day(3), day(3)));
}

#[test]
fn no_date_or_nonsense_is_no_hint() {
    for answer in [
        "{\"from\":null,\"to\":null}",
        "no date here",
        "{\"from\":\"last Saturday\"}",
        "{\"from\":\"2026-10-08\",\"to\":\"2026-10-06\"}",
        "} {",
    ] {
        assert_eq!(parse(answer, "UTC"), None, "{answer}");
    }
}
