//! Version 4: aggregate a complete native node locally, transport only requested combo rows.
use crate::turn::{query_turn, Step};
use crate::turn_reader::{load_turn, TurnQuery};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageQuery {
    pub schema_version: u32,
    pub native_sha256: String,
    pub input_sha256: String,
    pub path: Vec<Step>,
    pub max_memory_bytes: u64,
    pub player: usize,
    pub indices: Vec<usize>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub index: usize,
    pub combo: String,
    pub weight: f32,
    pub compatible_mass: f32,
    pub equity: f32,
    pub ev: f32,
    pub strategy: Option<Vec<f32>>,
    pub action_ev: Option<Vec<f32>>,
}
fn hand(combo: &str) -> String {
    let c = combo.as_bytes();
    let ranks = b"23456789TJQKA";
    let a = ranks.iter().position(|&r| r == c[0]).unwrap();
    let b = ranks.iter().position(|&r| r == c[2]).unwrap();
    let mut s = format!("{}{}", ranks[a.max(b)] as char, ranks[a.min(b)] as char);
    if a != b {
        s.push(if c[1] == c[3] { 's' } else { 'o' });
    }
    s
}
pub fn read_page(path: &Path, request: &PageQuery) -> Result<serde_json::Value, String> {
    if request.player > 1
        || request.indices.len() > 100
        || request.indices.iter().collect::<HashSet<_>>().len() != request.indices.len()
    {
        return Err("unsupported-input: player/indices page bounds".into());
    }
    let r = TurnQuery {
        schema_version: request.schema_version,
        native_sha256: request.native_sha256.clone(),
        input_sha256: request.input_sha256.clone(),
        path: request.path.clone(),
        max_memory_bytes: request.max_memory_bytes,
    };
    let version = request.schema_version;
    if ![4, 5].contains(&version) {
        return Err("unsupported-input: page version".into());
    }
    let mut game = load_turn(path, &r, version, if version == 5 { 16 } else { 128 })?;
    let node = query_turn(&mut game, &request.path)?;
    let p = request.player;
    let count = node.private_cards[p].len();
    if request.indices.iter().any(|&i| i >= count) {
        return Err("unsupported-input: combo index outside native range".into());
    }
    let mass: f64 = node.compatible_weights[p].iter().map(|&v| v as f64).sum();
    let actor = node.player == Some(p);
    let rows: Vec<Row> = request
        .indices
        .iter()
        .map(|&i| Row {
            index: i,
            combo: node.private_cards[p][i].clone(),
            weight: node.weights[p][i],
            compatible_mass: node.compatible_weights[p][i],
            equity: node.equity[p][i],
            ev: node.ev[p][i],
            strategy: actor.then(|| {
                (0..node.actions.len())
                    .map(|a| node.strategy[a * count + i])
                    .collect()
            }),
            action_ev: actor.then(|| {
                (0..node.actions.len())
                    .map(|a| node.action_ev[a * count + i])
                    .collect()
            }),
        })
        .collect();
    let mut classes: BTreeMap<String, f64> = BTreeMap::new();
    for i in 0..count {
        *classes.entry(hand(&node.private_cards[p][i])).or_default() +=
            node.compatible_weights[p][i] as f64;
    }
    let classes: Vec<_> = classes
        .into_iter()
        .map(|(hand, w)| serde_json::json!({"hand":hand,"probability":(mass>0.0).then(||w/mass)}))
        .collect();
    let frequencies: Vec<_> = node
        .actions
        .iter()
        .enumerate()
        .map(|(a, action)| {
            let freq = (mass > 0.0 && actor).then(|| {
                (0..count)
                    .map(|i| {
                        node.compatible_weights[p][i] as f64 * node.strategy[a * count + i] as f64
                    })
                    .sum::<f64>()
                    / mass
            });
            serde_json::json!({"action":action,"frequency":freq})
        })
        .collect();
    Ok(
        serde_json::json!({"schemaVersion":version,"path":node.path,"player":p,"counts":[node.private_cards[0].len(),node.private_cards[1].len()],
        "node":{"kind":node.kind,"board":node.board,"terminal":node.terminal,"player":node.player,"actions":node.actions,"chanceCards":node.chance_cards,"invested":node.invested},
        "rows":rows,"aggregate":{"denominator":mass,"frequencies":frequencies,"classes":classes}}),
    )
}
