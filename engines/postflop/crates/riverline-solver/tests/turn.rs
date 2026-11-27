use riverline_solver::turn::{build_turn, query_turn, run_turn, Step, TurnInput};

fn input(index: usize) -> TurnInput {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../benchmarks/solver/turn-cases.json"
    ))
    .unwrap();
    let mut value = manifest["cases"][index].clone();
    value["schemaVersion"] = 3.into();
    value["limits"] = manifest["limits"].clone();
    serde_json::from_value(value).unwrap()
}
#[test]
fn turn_physical_chance_mapping_and_zero_joint_mass_are_explicit() {
    let (mut game, _) = run_turn(input(0), || false, |_, _| {}).unwrap();
    let a = Step::Action { index: 0 };
    let chance = query_turn(&mut game, &[a.clone(), a.clone()]).unwrap();
    assert_eq!(chance.kind, "chance");
    assert_eq!(chance.chance_cards.len(), 48);
    assert!(
        (chance
            .chance_cards
            .iter()
            .map(|c| c.probability.unwrap())
            .sum::<f64>()
            - 1.0)
            .abs()
            < 1e-8
    );
    assert!(query_turn(&mut game, &[Step::Deal { card: "Ac".into() }]).is_err());
    assert!(query_turn(&mut game, &[a.clone(), a.clone(), a.clone()]).is_err());
    assert!(query_turn(
        &mut game,
        &[a.clone(), a.clone(), Step::Deal { card: "2c".into() }]
    )
    .is_err());
    let board = query_turn(
        &mut game,
        &[a.clone(), a.clone(), Step::Deal { card: "Ac".into() }],
    )
    .unwrap();
    assert_eq!(board.board, ["2c", "3d", "7h", "9s", "Ac"]);
    let mut zero = input(0);
    zero.ranges[0] = [("AcAd".into(), 1.0)].into();
    zero.ranges[1] = [("AcKh".into(), 1.0)].into();
    assert!(build_turn(&zero).is_err());
}
#[test]
fn turn_profile_rejects_implicit_clamping_and_enforces_resource_limits() {
    let mut value = input(2);
    value.turn_bets = [1500, 0]; // Existing 700 river bet must not silently become 500.
    assert!(build_turn(&value).is_err());
    let mut value = input(2);
    value.river_bets = [0, 2001];
    assert!(build_turn(&value).is_err());
    let mut value = input(2);
    value.board.push("Jc".into());
    assert!(build_turn(&value).is_err());
    let mut value = input(2);
    value.limits.max_memory_bytes = 1;
    assert!(run_turn(value, || false, |_, _| {}).is_err());
    assert!(run_turn(input(2), || true, |_, _| {}).is_err());
    let (mut game, _) = run_turn(input(4), || false, |_, _| {}).unwrap();
    let allin = query_turn(
        &mut game,
        &[Step::Action { index: 1 }, Step::Action { index: 1 }],
    )
    .unwrap();
    assert!(allin.terminal);
    assert_eq!(allin.board.len(), 4); // Exact river values already integrated, no future decisions.
    assert_eq!(allin.invested, [800, 800]);
}
