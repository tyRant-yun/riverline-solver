//! Thin, single-threaded river validation driver. The poker algorithm is upstream.
use postflop_solver::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

pub mod explicit;
pub mod flop;
pub mod native_reader;
pub mod turn;
pub mod turn_page;
pub mod turn_reader;

pub const ENGINE_COMMIT: &str = "9d1509fe5077d019825f833eed04b16d342dfda1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    pub max_iterations: u32,
    pub target_exploitability: f32,
    pub max_memory_bytes: u64,
    pub max_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategyLock {
    pub path: Vec<usize>,
    /// Combo -> action probabilities. Absent combos remain unlocked.
    pub allocations: BTreeMap<String, Vec<f32>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RiverInput {
    pub schema_version: u32,
    pub case_id: String,
    pub board: Vec<String>,
    /// Amounts are integer centibb. No amounts are silently rounded.
    pub pot: i32,
    pub stack: i32,
    pub ranges: [BTreeMap<String, f32>; 2],
    /// [OOP, IP], each [bet expression, raise expression], upstream syntax.
    pub sizes: [[String; 2]; 2],
    pub limits: Limits,
    #[serde(default)]
    pub locks: Vec<StrategyLock>,
}

fn parse_combo(combo: &str) -> Result<(u8, u8), String> {
    if combo.len() != 4 || !combo.is_ascii() {
        return Err(format!("invalid combo: {combo}"));
    }
    let a = card_from_str(&combo[..2])?;
    let b = card_from_str(&combo[2..])?;
    if a == b {
        return Err(format!("repeated card: {combo}"));
    }
    Ok((a.min(b), a.max(b)))
}

pub fn build(input: &RiverInput) -> Result<PostFlopGame, String> {
    if input.schema_version != 1 || input.board.len() != 5 {
        return Err("unsupported-input: only river schema 1 is supported".into());
    }
    if input.pot <= 0 || input.stack <= 0 || input.pot > 1_000_000 || input.stack > 1_000_000 {
        return Err("unsupported-input: pot/stack outside 1..1000000 centibb".into());
    }
    // R1 is deliberately the exact game covered by the independent oracle.
    // Reject other trees before upstream construction, not after allocating them.
    let percent = input.sizes[0][0]
        .strip_suffix('%')
        .ok_or("unsupported-input: R1 needs one OOP percentage bet")?;
    if percent.is_empty()
        || percent.len() > 8
        || !percent.chars().all(|c| c.is_ascii_digit() || c == '.')
        || !input.sizes[0][1].is_empty()
        || input.sizes[1] != ["", ""]
    {
        return Err(
            "unsupported-input: R1 only covers OOP check/bet and IP fold/call, no raises".into(),
        );
    }
    let percent: f64 = percent
        .parse()
        .map_err(|_| "unsupported-input: invalid percentage")?;
    let amount = input.pot as f64 * percent / 100.0;
    if !amount.is_finite()
        || amount < 1.0
        || amount > input.stack as f64
        || (amount - amount.round()).abs() > 1e-9
    {
        return Err(
            "unsupported-input: bet must be positive, within stack, and exact integer centibb"
                .into(),
        );
    }
    let bets = |player: usize| {
        BetSizeOptions::try_from((
            input.sizes[player][0].as_str(),
            input.sizes[player][1].as_str(),
        ))
    };
    // No implicit size merging or automatic all-in additions.
    let tree = ActionTree::new(TreeConfig {
        initial_state: BoardState::River,
        starting_pot: input.pot,
        effective_stack: input.stack,
        river_bet_sizes: [bets(0)?, bets(1)?],
        add_allin_threshold: 0.0,
        force_allin_threshold: 0.0,
        merging_threshold: 0.0,
        ..Default::default()
    })?;
    build_with_tree(input, tree)
}

pub(crate) fn build_with_tree(
    input: &RiverInput,
    tree: ActionTree,
) -> Result<PostFlopGame, String> {
    if input.pot <= 0 || input.stack <= 0 || input.pot > 1_000_000 || input.stack > 1_000_000 {
        return Err("unsupported-input: pot/stack outside 1..1000000 centibb".into());
    }
    if input.limits.max_iterations == 0
        || input.limits.max_iterations > 100_000
        || input.limits.max_seconds == 0
        || input.limits.max_seconds > 600
        || input.limits.max_memory_bytes > 2 * 1024 * 1024 * 1024
        || input.limits.max_memory_bytes == 0
        || !input.limits.target_exploitability.is_finite()
        || input.limits.target_exploitability < 0.0
    {
        return Err("unsupported-input: invalid resource/quality limits".into());
    }
    let board: Vec<u8> = input
        .board
        .iter()
        .map(|s| card_from_str(s))
        .collect::<Result<_, _>>()?;
    let unique: HashSet<u8> = board.iter().copied().collect();
    if unique.len() != board.len() || !(3..=5).contains(&board.len()) {
        return Err("unsupported-input: duplicate board card".into());
    }
    let mut ranges = [Range::new(), Range::new()];
    for (player, source) in input.ranges.iter().enumerate() {
        if source.is_empty() || source.len() > 1326 {
            return Err("unsupported-input: empty/oversized range".into());
        }
        let mut hands = Vec::new();
        let mut weights = Vec::new();
        let mut seen = HashSet::new();
        for (combo, &weight) in source {
            let hand = parse_combo(combo)?;
            if !seen.insert(hand) || !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
                return Err(format!(
                    "unsupported-input: duplicate combo or invalid weight: {combo}"
                ));
            }
            // Board-blocked zero-weight cells are harmless; positive blocked input is rejected.
            if weight > 0.0 && (unique.contains(&hand.0) || unique.contains(&hand.1)) {
                return Err(format!("unsupported-input: board-blocked combo: {combo}"));
            }
            hands.push(hand);
            weights.push(weight);
        }
        ranges[player] = Range::from_hands_weights(&hands, &weights)?;
    }
    let cards = CardConfig {
        range: ranges,
        flop: [board[0], board[1], board[2]],
        turn: board.get(3).copied().unwrap_or(NOT_DEALT),
        river: board.get(4).copied().unwrap_or(NOT_DEALT),
    };
    PostFlopGame::with_config(cards, tree)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub path: Vec<usize>,
    pub terminal: bool,
    pub player: Option<usize>,
    pub actions: Vec<String>,
    pub invested: [i32; 2],
    pub weights: [Vec<f32>; 2],
    pub compatible_weights: [Vec<f32>; 2],
    pub equity: [Vec<f32>; 2],
    pub ev: [Vec<f32>; 2],
    pub action_ev: Vec<f32>,
    pub strategy: Vec<f32>,
}

pub fn locate(game: &mut PostFlopGame, path: &[usize]) -> Result<(), String> {
    game.back_to_root();
    for &index in path {
        if game.is_terminal_node()
            || game.is_chance_node()
            || index >= game.available_actions().len()
        {
            return Err("unsupported-input: invalid river action path".into());
        }
        game.play(index);
    }
    Ok(())
}

pub fn query(game: &mut PostFlopGame, path: &[usize]) -> Result<Node, String> {
    locate(game, path)?;
    game.cache_normalized_weights();
    let terminal = game.is_terminal_node();
    let player = (!terminal).then(|| game.current_player());
    let actions = if terminal {
        vec![]
    } else {
        game.available_actions()
            .iter()
            .map(|a| format!("{a:?}"))
            .collect()
    };
    Ok(Node {
        path: path.to_vec(),
        terminal,
        player,
        actions,
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
        strategy: if terminal { vec![] } else { game.strategy() },
    })
}

pub fn lock(game: &mut PostFlopGame, locks: &[StrategyLock]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for constraint in locks {
        if !seen.insert(constraint.path.clone()) || constraint.allocations.is_empty() {
            return Err("unsupported-input: duplicate path or empty lock".into());
        }
        locate(game, &constraint.path)?;
        if game.is_terminal_node() {
            return Err("unsupported-input: cannot lock terminal".into());
        }
        let player = game.current_player();
        let hands = game.private_cards(player);
        let num_actions = game.available_actions().len();
        let mut values = vec![-1.0; hands.len() * num_actions];
        let mut seen_hands = HashSet::new();
        for (combo, probabilities) in &constraint.allocations {
            let parsed = parse_combo(combo)?;
            if !seen_hands.insert(parsed) {
                return Err("unsupported-input: duplicate locked combo alias".into());
            }
            let i = hands
                .iter()
                .position(|&h| h == parsed)
                .ok_or("unsupported-input: locked combo is absent")?;
            if probabilities.len() != num_actions
                || probabilities
                    .iter()
                    .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
                || (probabilities.iter().sum::<f32>() - 1.0).abs() > 1e-6
            {
                return Err(
                    "unsupported-input: lock must specify a complete probability column".into(),
                );
            }
            for (a, &probability) in probabilities.iter().enumerate() {
                values[a * hands.len() + i] = probability;
            }
        }
        game.lock_current_strategy(&values);
    }
    game.back_to_root();
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report<T = RiverInput> {
    pub engine_commit: String,
    pub algorithm: String,
    pub input: T,
    pub iterations: u32,
    pub exploitability: f32,
    pub centered_root_ev: [f32; 2],
    pub memory_bytes: u64,
    pub elapsed_ms: u128,
    pub private_cards: [Vec<String>; 2],
    pub nodes: Vec<Node>,
    pub stop_reason: String,
    pub quality_scope: String,
}

/// Run real upstream DCFR steps. Cancellation is checked between iterations.
pub fn run(
    input: RiverInput,
    cancelled: impl Fn() -> bool,
    progress: impl FnMut(u32, f32),
) -> Result<(PostFlopGame, Report), String> {
    let start = Instant::now();
    let game = build(&input)?;
    let limits = input.limits.clone();
    let locks = input.locks.clone();
    run_prepared(input, game, limits, &locks, start, cancelled, progress)
}

pub(crate) fn run_prepared<T>(
    input: T,
    mut game: PostFlopGame,
    limits: Limits,
    locks: &[StrategyLock],
    start: Instant,
    cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(u32, f32),
) -> Result<(PostFlopGame, Report<T>), String> {
    let memory_bytes = game.memory_usage().0;
    if memory_bytes > limits.max_memory_bytes {
        return Err("budget-exceeded: estimated allocation".into());
    }
    game.allocate_memory(false);
    lock(&mut game, locks)?;
    let mut exploitability = compute_exploitability(&game);
    let mut iterations = 0;
    while iterations < limits.max_iterations && exploitability > limits.target_exploitability {
        if cancelled() {
            return Err("cancelled".into());
        }
        if start.elapsed().as_secs() >= limits.max_seconds {
            return Err("budget-exceeded: time".into());
        }
        solve_step(&game, iterations);
        iterations += 1;
        if iterations % 10 == 0 || iterations == limits.max_iterations {
            exploitability = compute_exploitability(&game);
            progress(iterations, exploitability);
        }
    }
    if cancelled() {
        return Err("cancelled".into());
    }
    let centered_root_ev = compute_current_ev(&game);
    finalize(&mut game);
    let private_cards = [
        holes_to_strings(game.private_cards(0))?,
        holes_to_strings(game.private_cards(1))?,
    ];
    let stop_reason = if exploitability <= limits.target_exploitability {
        "quality-target"
    } else {
        "iteration-limit"
    }
    .into();
    let quality_scope = if locks.is_empty() {
        "unraked-two-player-zero-sum-half-nashconv"
    } else {
        "locked-game-upstream-metric-not-independently-validated"
    }
    .into();
    let report = Report {
        engine_commit: ENGINE_COMMIT.into(),
        algorithm: "upstream-dcfr-gamma3-v1".into(),
        input,
        iterations,
        exploitability,
        centered_root_ev,
        memory_bytes,
        elapsed_ms: start.elapsed().as_millis(),
        private_cards,
        nodes: vec![],
        stop_reason,
        quality_scope,
    };
    Ok((game, report))
}

/// Small-tree R1 evidence only. Production queries must not export a whole tree.
pub fn collect(game: &mut PostFlopGame) -> Result<Vec<Node>, String> {
    let mut paths = vec![vec![]];
    let mut nodes = Vec::new();
    while let Some(path) = paths.pop() {
        if nodes.len() >= 2000 {
            return Err("unsupported-input: R1 report tree >2000 nodes".into());
        }
        let node = query(game, &path)?;
        for i in (0..node.actions.len()).rev() {
            let mut child = path.clone();
            child.push(i);
            paths.push(child);
        }
        nodes.push(node);
    }
    Ok(nodes)
}
