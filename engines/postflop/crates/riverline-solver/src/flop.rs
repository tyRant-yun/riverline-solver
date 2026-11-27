//! AF04 bounded three-street adapter over the fixed upstream solver.
use crate::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlopInput {
    pub schema_version: u32,
    pub case_id: String,
    pub board: Vec<String>,
    pub pot: i32,
    pub stack: i32,
    pub ranges: [BTreeMap<String, f32>; 2],
    pub flop_bets: [i32; 2],
    pub turn_bets: [i32; 2],
    pub river_bets: [i32; 2],
    pub limits: Limits,
    #[serde(default)]
    pub locks: Vec<StrategyLock>,
}

pub fn build_flop(input: &FlopInput) -> Result<PostFlopGame, String> {
    if !input.locks.is_empty()
        || input.schema_version != 5
        || input.board.len() != 3
        || input.case_id.is_empty()
        || input.ranges.iter().any(|r| r.len() > 1326)
    {
        return Err("unsupported-input: isolated flop/5 input".into());
    }
    for f in input.flop_bets {
        for t in input.turn_bets {
            for r in input.river_bets {
                if [f, t, r].iter().any(|&b| b < 0 || b > input.stack)
                    || (f < input.stack && t > input.stack - f)
                    || (f + t < input.stack && r > input.stack - f - t)
                {
                    return Err("unsupported-input: implicit continuation size clamping".into());
                }
            }
        }
    }
    let options = |n: i32| BetSizeOptions {
        bet: if n == 0 {
            vec![]
        } else if n == input.stack {
            vec![BetSize::AllIn]
        } else {
            vec![BetSize::Additive(n, 0)]
        },
        raise: vec![],
    };
    let tree = ActionTree::new(TreeConfig {
        initial_state: BoardState::Flop,
        starting_pot: input.pot,
        effective_stack: input.stack,
        flop_bet_sizes: input.flop_bets.map(options),
        turn_bet_sizes: input.turn_bets.map(options),
        river_bet_sizes: input.river_bets.map(options),
        add_allin_threshold: 0.0,
        force_allin_threshold: 0.0,
        merging_threshold: 0.0,
        ..Default::default()
    })?;
    build_with_tree(
        &RiverInput {
            schema_version: 1,
            case_id: input.case_id.clone(),
            board: input.board.clone(),
            pot: input.pot,
            stack: input.stack,
            ranges: input.ranges.clone(),
            sizes: Default::default(),
            limits: input.limits.clone(),
            locks: vec![],
        },
        tree,
    )
}

pub fn run_flop(input: FlopInput) -> Result<(PostFlopGame, Report<FlopInput>), String> {
    run_flop_controlled(input, || false, |_, _| {})
}
pub fn run_flop_controlled(
    input: FlopInput,
    cancelled: impl Fn() -> bool,
    progress: impl FnMut(u32, f32),
) -> Result<(PostFlopGame, Report<FlopInput>), String> {
    let start = Instant::now();
    let game = build_flop(&input)?;
    let limits = input.limits.clone();
    run_prepared(input, game, limits, &[], start, cancelled, progress)
}
