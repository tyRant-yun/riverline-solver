//! Exact, bounded two-street adapter. Rules/CFR/evaluation remain upstream.
use crate::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnInput {
    pub schema_version: u32,
    pub case_id: String,
    pub board: Vec<String>,
    pub pot: i32,
    pub stack: i32,
    pub ranges: [BTreeMap<String, f32>; 2],
    /// Zero or one explicit constant centibb bet per side/street. No raises.
    pub turn_bets: [i32; 2],
    pub river_bets: [i32; 2],
    pub limits: Limits,
    #[serde(default)]
    pub locks: Vec<StrategyLock>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Step {
    Action { index: usize },
    Deal { card: String },
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChanceCard {
    pub card: String,
    pub probability: Option<f64>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnNode {
    pub private_cards: [Vec<String>; 2],
    pub path: Vec<Step>,
    pub kind: String,
    pub board: Vec<String>,
    pub terminal: bool,
    pub player: Option<usize>,
    pub actions: Vec<String>,
    pub chance_cards: Vec<ChanceCard>,
    pub invested: [i32; 2],
    pub weights: [Vec<f32>; 2],
    pub compatible_weights: [Vec<f32>; 2],
    pub equity: [Vec<f32>; 2],
    pub ev: [Vec<f32>; 2],
    pub action_ev: Vec<f32>,
    pub strategy: Vec<f32>,
}
pub fn build_turn(input: &TurnInput) -> Result<PostFlopGame, String> {
    if !input.locks.is_empty()
        || ![3, 4].contains(&input.schema_version)
        || input.board.len() != 4
        || input.case_id.is_empty()
        || input.case_id.len() > 200
        || input.limits.max_memory_bytes > 256 * 1024 * 1024
        || input.limits.max_seconds > 120
        || input
            .ranges
            .iter()
            .any(|r| r.values().filter(|&&w| w > 0.0).count() > 200)
        || input
            .turn_bets
            .iter()
            .chain(input.river_bets.iter())
            .any(|&n| n < 0 || n > input.stack)
        || input.river_bets.iter().any(|&r| {
            r > 0
                && input
                    .turn_bets
                    .iter()
                    .any(|&t| t < input.stack && t + r > input.stack)
        })
    {
        return Err("unsupported-input: invalid turn/3 model, bet or budget".into());
    }
    let options = |amount: i32| BetSizeOptions {
        bet: if amount == 0 {
            vec![]
        } else if amount == input.stack {
            vec![BetSize::AllIn]
        } else {
            vec![BetSize::Additive(amount, 0)]
        },
        raise: vec![],
    };
    let tree = ActionTree::new(TreeConfig {
        initial_state: BoardState::Turn,
        starting_pot: input.pot,
        effective_stack: input.stack,
        turn_bet_sizes: input.turn_bets.map(options),
        river_bet_sizes: input.river_bets.map(options),
        add_allin_threshold: 0.0,
        force_allin_threshold: 0.0,
        merging_threshold: 0.0,
        ..Default::default()
    })?;
    let common = RiverInput {
        schema_version: 1,
        case_id: input.case_id.clone(),
        board: input.board.clone(),
        pot: input.pot,
        stack: input.stack,
        ranges: input.ranges.clone(),
        sizes: Default::default(),
        limits: input.limits.clone(),
        locks: vec![],
    };
    build_with_tree(&common, tree)
}
pub fn run_turn(
    input: TurnInput,
    cancelled: impl Fn() -> bool,
    progress: impl FnMut(u32, f32),
) -> Result<(PostFlopGame, Report<TurnInput>), String> {
    let start = Instant::now();
    let game = build_turn(&input)?;
    let limits = input.limits.clone();
    run_prepared(input, game, limits, &[], start, cancelled, progress)
}
pub fn locate_turn(game: &mut PostFlopGame, path: &[Step]) -> Result<(), String> {
    if path.len() > 24 {
        return Err("unsupported-input: turn path too deep".into());
    }
    game.back_to_root();
    for step in path {
        if game.is_terminal_node() {
            return Err("unsupported-input: terminal path".into());
        }
        match step {
            Step::Action { index }
                if !game.is_chance_node() && *index < game.available_actions().len() =>
            {
                game.play(*index)
            }
            Step::Deal { card } if game.is_chance_node() => {
                let id = card_from_str(card)?;
                if game.possible_cards() & (1u64 << id) == 0 {
                    return Err("unsupported-input: impossible river".into());
                }
                game.play(id as usize);
            }
            _ => return Err("unsupported-input: action/deal path kind mismatch".into()),
        }
    }
    Ok(())
}
pub fn query_turn(game: &mut PostFlopGame, path: &[Step]) -> Result<TurnNode, String> {
    locate_turn(game, path)?;
    game.cache_normalized_weights();
    let terminal = game.is_terminal_node();
    let chance = game.is_chance_node();
    let player = (!terminal && !chance).then(|| game.current_player());
    let actions = if player.is_some() {
        game.available_actions()
            .iter()
            .map(|a| format!("{a:?}"))
            .collect()
    } else {
        vec![]
    };
    let mut chance_cards = vec![];
    if chance {
        let hands = [game.private_cards(0), game.private_cards(1)];
        let weights = [game.weights(0), game.weights(1)];
        let mut mass = 0.0;
        let mut cards = [0.0; 52];
        for (i, &(a, b)) in hands[0].iter().enumerate() {
            for (j, &(c, d)) in hands[1].iter().enumerate() {
                if a == c || a == d || b == c || b == d {
                    continue;
                }
                let w = weights[0][i] as f64 * weights[1][j] as f64;
                mass += w;
                for r in 0..52u8 {
                    if r != a && r != b && r != c && r != d {
                        cards[r as usize] += w;
                    }
                }
            }
        }
        for id in 0..52u8 {
            if game.possible_cards() & (1u64 << id) != 0 {
                chance_cards.push(ChanceCard {
                    card: card_to_string(id)?,
                    probability: (mass > 0.0).then(|| {
                        cards[id as usize] / ((48 - game.current_board().len()) as f64 * mass)
                    }),
                });
            }
        }
    }
    Ok(TurnNode {
        private_cards: [
            holes_to_strings(game.private_cards(0))?,
            holes_to_strings(game.private_cards(1))?,
        ],
        path: path.to_vec(),
        kind: if terminal {
            "terminal"
        } else if chance {
            "chance"
        } else {
            "decision"
        }
        .into(),
        board: game
            .current_board()
            .iter()
            .map(|&c| card_to_string(c))
            .collect::<Result<_, _>>()?,
        terminal,
        player,
        actions,
        chance_cards,
        invested: game.total_bet_amount(),
        weights: [game.weights(0).to_vec(), game.weights(1).to_vec()],
        compatible_weights: [
            game.normalized_weights(0).to_vec(),
            game.normalized_weights(1).to_vec(),
        ],
        equity: [game.equity(0), game.equity(1)],
        ev: [game.expected_values(0), game.expected_values(1)],
        action_ev: player
            .map(|p| game.expected_values_detail(p))
            .unwrap_or_default(),
        strategy: if player.is_some() {
            game.strategy()
        } else {
            vec![]
        },
    })
}
/// Independent-benchmark export only. Product queries use one typed path.
pub fn collect_turn(game: &mut PostFlopGame) -> Result<Vec<TurnNode>, String> {
    let mut pending = vec![vec![]];
    let mut nodes = vec![];
    while let Some(path) = pending.pop() {
        if nodes.len() >= 6000 {
            return Err("benchmark-only: turn export >6000 nodes".into());
        }
        let node = query_turn(game, &path)?;
        for index in (0..node.actions.len()).rev() {
            let mut p = path.clone();
            p.push(Step::Action { index });
            pending.push(p);
        }
        for chance in node.chance_cards.iter().rev() {
            let mut p = path.clone();
            p.push(Step::Deal {
                card: chance.card.clone(),
            });
            pending.push(p);
        }
        nodes.push(node);
    }
    Ok(nodes)
}
