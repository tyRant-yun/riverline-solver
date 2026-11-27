//! Exact river tree additions. Upstream validates legality; no CFR is implemented here.
use crate::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RiverAction {
    Check,
    Fold,
    Call,
    Bet { amount: i32 },
    Raise { amount: i32 },
    AllIn { amount: i32 },
}

impl RiverAction {
    fn native(&self) -> Action {
        match *self {
            Self::Check => Action::Check,
            Self::Fold => Action::Fold,
            Self::Call => Action::Call,
            Self::Bet { amount } => Action::Bet(amount),
            Self::Raise { amount } => Action::Raise(amount),
            Self::AllIn { amount } => Action::AllIn(amount),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExplicitRiverInput {
    pub schema_version: u32,
    pub case_id: String,
    pub board: Vec<String>,
    pub pot: i32,
    pub stack: i32,
    pub ranges: [BTreeMap<String, f32>; 2],
    /// Topologically ordered additions to the mandatory check/fold/call skeleton.
    /// Bet/raise/all-in amounts are total river investment (centibb), never increments.
    pub tree_lines: Vec<Vec<RiverAction>>,
    pub limits: Limits,
    #[serde(default)]
    pub locks: Vec<StrategyLock>,
}

pub fn build_explicit(input: &ExplicitRiverInput) -> Result<PostFlopGame, String> {
    if input.schema_version != 2
        || input.board.len() != 5
        || input.pot <= 0
        || input.pot > 1_000_000
        || input.stack <= 0
        || input.stack > 1_000_000
        || input.tree_lines.len() > 128
        || input.case_id.is_empty()
        || input.case_id.len() > 200
        || input.limits.max_memory_bytes > 256 * 1024 * 1024
        || input.limits.max_seconds > 120
        || input.locks.len() > 128
        || input.locks.iter().any(|l| l.path.len() > 16)
    {
        return Err("unsupported-input: invalid explicit river schema/budget".into());
    }
    let mut tree = ActionTree::new(TreeConfig {
        initial_state: BoardState::River,
        starting_pot: input.pot,
        effective_stack: input.stack,
        add_allin_threshold: 0.0,
        force_allin_threshold: 0.0,
        merging_threshold: 0.0,
        ..Default::default()
    })?;
    for line in &input.tree_lines {
        if line.is_empty() || line.len() > 16 {
            return Err("unsupported-input: line length outside 1..16".into());
        }
        let native: Vec<Action> = line.iter().map(RiverAction::native).collect();
        // Upstream replaces Bet(stack)/Raise(stack) with AllIn; reject this implicit change.
        if line.iter().any(|a| match a {
            RiverAction::Bet { amount } | RiverAction::Raise { amount } => {
                *amount <= 0 || *amount >= input.stack
            }
            RiverAction::AllIn { amount } => *amount != input.stack,
            _ => false,
        }) {
            return Err(
                "unsupported-input: invalid amount or implicit all-in; use all-in explicitly"
                    .into(),
            );
        }
        tree.add_line(&native)
            .map_err(|e| format!("unsupported-input: {e}"))?;
        tree.apply_history(&native)?;
        tree.back_to_root();
    }
    let common = RiverInput {
        schema_version: 1,
        case_id: input.case_id.clone(),
        board: input.board.clone(),
        pot: input.pot,
        stack: input.stack,
        ranges: input.ranges.clone(),
        sizes: Default::default(),
        limits: input.limits.clone(),
        locks: input.locks.clone(),
    };
    build_with_tree(&common, tree)
}

pub fn run_explicit(
    input: ExplicitRiverInput,
    cancelled: impl Fn() -> bool,
    progress: impl FnMut(u32, f32),
) -> Result<(PostFlopGame, Report<ExplicitRiverInput>), String> {
    let start = Instant::now();
    let game = build_explicit(&input)?;
    let limits = input.limits.clone();
    let locks = input.locks.clone();
    let (game, mut report) = run_prepared(input, game, limits, &locks, start, cancelled, progress)?;
    if !locks.is_empty() {
        report.quality_scope = "fixed-constraints-half-constrained-nashconv".into();
    }
    Ok((game, report))
}
