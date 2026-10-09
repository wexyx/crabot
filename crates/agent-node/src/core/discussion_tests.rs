use super::discussion::Discussion;

fn discussion() -> Discussion {
    Discussion::new(["a", "b", "c"].map(str::to_owned))
}

#[test]
fn consensus_requires_every_member_on_the_current_proposal() {
    let mut d = discussion();
    d.record("a", "采用方案一。");
    d.record("b", "同意当前结论，无补充。");
    assert!(!d.concluded());
    d.record("c", "采用方案二，还需补充测试。");
    assert!(!d.concluded());
    d.record("a", "同意当前结论，无补充。");
    assert!(!d.concluded());
    d.record("b", "同意当前结论，无补充。");
    assert!(d.concluded());
}

#[test]
fn unrelated_members_can_yield_but_not_approve() {
    let mut d = discussion();
    d.record("a", "采用方案一。");
    d.record("b", "本轮让出。");
    assert!(!d.concluded());
    d.record("c", "同意当前结论，无补充。");
    assert!(d.concluded());
    assert!(!d.unclaimed());
    d.record("c", "改用方案二。");
    assert!(
        !d.concluded(),
        "an earlier abstention does not cover new work"
    );
}

#[test]
fn all_yield_is_unclaimed_not_consensus() {
    let mut d = discussion();
    for member in ["a", "b", "c"] {
        d.record(member, "本轮让出。");
    }
    assert!(d.unclaimed());
    assert!(!d.concluded());
}

#[test]
fn empty_quoted_negative_or_unknown_votes_do_not_end_discussion() {
    for answer in [
        "",
        "不同意当前结论，无补充。",
        "引用：同意当前结论，无补充。",
        "同意当前结论，无补充。但还需要测试",
        "本轮让出。但必须先修复问题",
    ] {
        let mut d = discussion();
        d.record("a", "方案一");
        d.record("b", "同意当前结论，无补充。");
        d.record("c", answer);
        assert!(!d.concluded(), "{answer}");
    }
    let mut d = discussion();
    d.record("a", "方案一");
    d.record("b", "同意当前结论，无补充。");
    d.record("other", "同意当前结论，无补充。");
    assert!(!d.concluded());
}

#[test]
fn confirmations_without_a_proposal_never_form_consensus() {
    let mut d = discussion();
    for member in ["a", "b", "c"] {
        d.record(member, "同意当前结论，无补充。");
    }
    assert!(!d.concluded());
}
