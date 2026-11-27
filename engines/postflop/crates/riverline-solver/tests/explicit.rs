use riverline_solver::explicit::{build_explicit, run_explicit, ExplicitRiverInput, RiverAction};
use riverline_solver::{collect, query};

fn input(index: usize) -> ExplicitRiverInput {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../benchmarks/solver/river-tree-cases.json"
    ))
    .unwrap();
    let mut case = manifest["cases"][index].clone();
    case["schemaVersion"] = 2.into();
    case["limits"] = manifest["limits"].clone();
    serde_json::from_value(case).unwrap()
}

#[test]
fn explicit_tree_covers_multiple_sizes_ip_bets_and_reraises() {
    let (mut game, report) = run_explicit(input(0), || false, |_, _| {}).unwrap();
    assert_eq!(report.input.tree_lines.len(), 3);
    assert_eq!(
        query(&mut game, &[]).unwrap().actions,
        ["Check", "Bet(500)", "Bet(1000)"]
    );
    assert_eq!(
        query(&mut game, &[0]).unwrap().actions,
        ["Check", "Bet(500)"]
    );
    assert_eq!(collect(&mut game).unwrap().len(), 12);
    let (mut game, _) = run_explicit(input(1), || false, |_, _| {}).unwrap();
    assert_eq!(
        query(&mut game, &[1, 2]).unwrap().actions,
        ["Fold", "Call", "AllIn(2000)"]
    );
    assert_eq!(
        query(&mut game, &[1, 2, 2, 1]).unwrap().invested,
        [2000, 2000]
    );
}

#[test]
fn rejects_implicit_changes_illegal_raises_duplicates_and_orphans() {
    let mut i = input(0);
    i.tree_lines[0][0] = RiverAction::Bet { amount: 2000 };
    assert!(build_explicit(&i).is_err());
    let mut i = input(1);
    i.tree_lines[1][1] = RiverAction::Raise { amount: 750 };
    assert!(build_explicit(&i).is_err());
    let mut i = input(0);
    i.tree_lines.push(i.tree_lines[0].clone());
    assert!(build_explicit(&i).is_err());
    let mut i = input(1);
    i.tree_lines.reverse();
    assert!(build_explicit(&i).is_err());
    let mut i = input(0);
    i.pot = i32::MAX;
    assert!(build_explicit(&i).is_err());
    let mut i = input(0);
    i.limits.max_memory_bytes = 1;
    assert!(run_explicit(i, || false, |_, _| {}).is_err());
    let mut i = input(4);
    i.locks[0]
        .allocations
        .insert("AdAc".into(), vec![0.75, 0.25]);
    assert!(run_explicit(i, || false, |_, _| {})
        .err()
        .unwrap()
        .contains("duplicate locked combo"));
}

#[test]
fn mixed_lock_columns_and_short_allin_are_preserved() {
    let (mut game, _) = run_explicit(input(5), || false, |_, _| {}).unwrap();
    let node = query(&mut game, &[1, 2]).unwrap();
    let cards = postflop_solver::holes_to_strings(game.private_cards(0)).unwrap();
    let aa = cards.iter().position(|c| c == "AdAc").unwrap();
    for (a, expected) in [0.1, 0.2, 0.7].iter().enumerate() {
        assert!((node.strategy[a * cards.len() + aa] - expected).abs() < 1e-6);
    }
    let (mut game, _) = run_explicit(input(3), || false, |_, _| {}).unwrap();
    assert_eq!(query(&mut game, &[1, 2]).unwrap().actions, ["Fold", "Call"]);
    assert_eq!(query(&mut game, &[1, 2, 1]).unwrap().invested, [800, 800]);
}
