use riverline_solver::{build, collect, query, run, RiverInput, StrategyLock};
use std::collections::BTreeMap;

fn input() -> RiverInput {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../benchmarks/solver/river-cases.json"
    ))
    .unwrap();
    let mut case = manifest["cases"][0].clone();
    case["schemaVersion"] = 1.into();
    case["limits"] = manifest["limits"].clone();
    serde_json::from_value(case).unwrap()
}

#[test]
fn rejects_invalid_boards_duplicate_combos_and_nonriver() {
    let mut i = input();
    i.board[0] = i.board[1].clone();
    assert!(build(&i).is_err());
    let mut i = input();
    i.board.pop();
    assert!(build(&i).is_err());
    let mut i = input();
    i.ranges[0].insert("AdAc".into(), 0.5);
    assert!(build(&i).is_err());
    let mut i = input();
    i.ranges[0].insert("2cAc".into(), 1.0);
    assert!(build(&i).is_err());
}

#[test]
fn zero_weight_is_not_a_live_private_hand() {
    let mut i = input();
    i.ranges[0].insert("QhQd".into(), 0.0);
    let game = build(&i).unwrap();
    assert_eq!(game.private_cards(0).len(), 2);
}

#[test]
fn rejects_unvalidated_trees_and_rounding_instead_of_changing_input() {
    let mut i = input();
    i.sizes[0][0] = "33.33%".into();
    assert!(build(&i).is_err());
    let mut i = input();
    i.sizes[1][0] = "50%".into();
    assert!(build(&i).is_err());
    let mut i = input();
    i.limits.max_memory_bytes = 1;
    assert!(run(i, || false, |_, _| {})
        .err()
        .unwrap()
        .contains("budget-exceeded"));
}

#[test]
fn cancelling_real_steps_does_not_produce_a_solution() {
    let checks = std::cell::Cell::new(0);
    let result = run(
        input(),
        || {
            checks.set(checks.get() + 1);
            checks.get() > 50
        },
        |_, _| {},
    );
    assert_eq!(result.err().unwrap(), "cancelled");
    assert_eq!(checks.get(), 51);
}

#[test]
fn locks_apply_to_new_game_and_queries_reset_cursor() {
    let mut i = input();
    i.locks = vec![StrategyLock {
        path: vec![],
        allocations: BTreeMap::from([("AcAd".into(), vec![1.0, 0.0])]),
    }];
    let (mut game, _) = run(i, || false, |_, _| {}).unwrap();
    let root = query(&mut game, &[]).unwrap();
    let cards = postflop_solver::holes_to_strings(game.private_cards(0)).unwrap();
    let aa = cards.iter().position(|c| c == "AdAc").unwrap();
    assert_eq!(root.strategy[aa], 1.0);
    assert_eq!(root.strategy[aa + cards.len()], 0.0);
    let _ = query(&mut game, &[1]).unwrap();
    let again = query(&mut game, &[]).unwrap();
    assert_eq!(root.strategy, again.strategy);
    assert!(query(&mut game, &[99]).is_err());
    assert_eq!(query(&mut game, &[]).unwrap().strategy, root.strategy);
}

#[test]
fn native_roundtrip_and_corruption_are_distinct_from_resume() {
    let (mut game, _) = run(input(), || false, |_, _| {}).unwrap();
    let nodes = collect(&mut game).unwrap();
    let mut buffer = vec![];
    postflop_solver::save_data_into_std_write(&game, "test", &mut buffer, None).unwrap();
    let (mut loaded, memo): (postflop_solver::PostFlopGame, String) =
        postflop_solver::load_data_from_std_read(
            &mut buffer.as_slice(),
            Some(2 * 1024 * 1024 * 1024),
        )
        .unwrap();
    assert_eq!(memo, "test");
    assert_eq!(
        serde_json::to_value(nodes).unwrap(),
        serde_json::to_value(collect(&mut loaded).unwrap()).unwrap()
    );
    buffer[0] ^= 1;
    let result: Result<(postflop_solver::PostFlopGame, String), _> =
        postflop_solver::load_data_from_std_read(
            &mut buffer.as_slice(),
            Some(2 * 1024 * 1024 * 1024),
        );
    assert!(result.is_err());
}
