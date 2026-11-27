use riverline_solver::flop::{build_flop, run_flop, FlopInput};
use riverline_solver::turn::{query_turn, Step};

fn input() -> FlopInput {
    serde_json::from_value(serde_json::json!({
        "schemaVersion":5,"caseId":"flop-test","board":["2c","7d","Jh"],"pot":200,"stack":2000,
        "ranges":[{"AcAd":1.0},{"KcKd":1.0}],"flopBets":[0,0],"turnBets":[0,0],"riverBets":[0,0],
        "limits":{"maxIterations":100,"targetExploitability":0.0001,"maxMemoryBytes":268435456,"maxSeconds":30}
    })).unwrap()
}

#[test]
fn flop_two_chance_streets_have_physical_card_probabilities_and_no_future_information() {
    let (mut game, _) = run_flop(input()).unwrap();
    let check = Step::Action { index: 0 };
    let chance = query_turn(&mut game, &[check.clone(), check.clone()]).unwrap();
    assert_eq!(chance.board.len(), 3);
    assert_eq!(chance.kind, "chance");
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
    assert_eq!(
        chance
            .chance_cards
            .iter()
            .find(|c| c.card == "Ac")
            .and_then(|c| c.probability)
            .unwrap_or(0.0),
        0.0
    );
    let river = query_turn(
        &mut game,
        &[
            check.clone(),
            check.clone(),
            Step::Deal { card: "3s".into() },
            check.clone(),
            check,
        ],
    )
    .unwrap();
    assert_eq!(river.board.len(), 4);
    assert!(
        (river
            .chance_cards
            .iter()
            .map(|c| c.probability.unwrap())
            .sum::<f64>()
            - 1.0)
            .abs()
            < 1e-8
    );
}

#[test]
fn flop_input_rejects_wrong_board_silent_clamping_and_memory_overrun() {
    let mut i = input();
    i.board.push("3s".into());
    assert!(build_flop(&i).is_err());
    let mut i = input();
    i.flop_bets = [1900, 0];
    i.turn_bets = [200, 0];
    assert!(build_flop(&i).is_err());
    let mut i = input();
    i.limits.max_memory_bytes = 1;
    assert!(run_flop(i).is_err());
}
