use super::*;

fn roster() -> Vec<Member> {
    ["a", "b", "c"]
        .into_iter()
        .map(|id| Member {
            path: vec![id.into()],
            role: "worker".into(),
        })
        .collect()
}
fn owner() -> Turn {
    Turn {
        member: 0,
        instruction: String::new(),
        consultation: false,
    }
}

#[test]
fn peers_reply_before_control_returns_to_the_owner() {
    let mut schedule = DiscussionSchedule::new(3, 2);
    schedule
        .consult(&owner(), "@b Which API?\n@c What tests?", &roster())
        .unwrap();
    let b = schedule.next().unwrap();
    assert_eq!(b.member, 1);
    assert!(b.consultation);
    assert!(
        b.instruction
            .contains("NOT a new human instruction or authorization")
    );
    assert_eq!(schedule.next().unwrap().member, 2);
    let resume = schedule.next().unwrap();
    assert_eq!(resume.member, 0);
    assert!(!resume.consultation);
    assert!(!schedule.pending());
}

#[test]
fn nested_questions_return_to_each_caller_without_changing_ownership() {
    let mut schedule = DiscussionSchedule::new(3, 2);
    schedule
        .consult(&owner(), "@b Which API?", &roster())
        .unwrap();
    let b = schedule.next().unwrap();
    schedule
        .consult(&b, "@c Which version?", &roster())
        .unwrap();
    assert_eq!(schedule.next().unwrap().member, 2);
    let b = schedule.next().unwrap();
    assert_eq!(b.member, 1);
    assert!(b.consultation);
    assert_eq!(schedule.next().unwrap().member, 0);
}

#[test]
fn prose_code_self_unknown_and_empty_addresses_do_not_schedule_work() {
    let mut schedule = DiscussionSchedule::new(3, 2);
    schedule.consult(&owner(), "As @b said\n> @b quoted\n```text\n@b code\n```\n@a self\n@stranger hello\n@中文 then text\n@b：", &roster()).unwrap();
    assert!(!schedule.pending());
    let ambiguous = vec![
        Member {
            path: vec!["node1".into(), "b".into()],
            role: "worker".into(),
        },
        Member {
            path: vec!["node2".into(), "b".into()],
            role: "worker".into(),
        },
    ];
    assert!(mentions::find("@b question", &ambiguous).is_empty());
    assert_eq!(
        mentions::find("@node2/b question", &ambiguous)[0].path,
        ambiguous[1].path
    );
}

#[test]
fn repeated_questions_and_total_work_are_bounded() {
    let mut schedule = DiscussionSchedule::new(3, 2);
    schedule
        .consult(&owner(), "@b Which API?", &roster())
        .unwrap();
    assert!(
        schedule
            .consult(&owner(), "@b Which  API?", &roster())
            .is_err()
    );
    schedule
        .consult(&owner(), "@b Which version?", &roster())
        .unwrap();
    assert!(
        schedule
            .consult(&owner(), "@b Another question?", &roster())
            .is_err()
    );
    schedule.remaining = 1;
    assert!(schedule.consult(&owner(), "@c Test?", &roster()).is_err());
}
