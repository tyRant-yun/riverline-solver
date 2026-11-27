//! Internal, bounded one-node reader; never returns a complete tree.
use crate::*;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeQuery {
    pub schema_version: u32,
    pub native_sha256: String,
    #[serde(default)]
    pub input_sha256: Option<String>,
    pub path: Vec<usize>,
    pub player: usize,
    pub offset: usize,
    pub limit: usize,
    pub max_memory_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub engine_index: usize,
    pub combo: String,
    pub own_path_weight: f32,
    pub compatible_weight: f32,
    pub equity: Option<f32>,
    pub ev: Option<f32>,
    pub strategy: Option<Vec<f32>>,
    pub action_ev: Option<Vec<f32>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePage {
    pub schema_version: u32,
    pub native_sha256: String,
    pub path: Vec<usize>,
    pub terminal: bool,
    pub acting_player: Option<usize>,
    pub player: usize,
    pub actions: Vec<String>,
    pub invested: [i32; 2],
    pub unit: String,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub rows: Vec<Row>,
}

pub fn read_page(path: &Path, request: &NativeQuery) -> Result<NodePage, String> {
    if request.schema_version != 1
        || request.path.len() > 16
        || request.player > 1
        || request.limit == 0
        || request.limit > 100
        || request.offset > 1326
        || request.max_memory_bytes == 0
        || request.max_memory_bytes > 256 * 1024 * 1024
        || request.native_sha256.len() != 64
        || !request
            .native_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("unsupported-input: invalid native query".into());
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.file_type().is_file() || meta.len() > 256 * 1024 * 1024 {
        return Err("artifact-error: invalid native file".into());
    }
    // Hash and deserialize the same open handle, avoiding a second path lookup.
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if format!("{:x}", hash.finalize()) != request.native_sha256 {
        return Err("artifact-error: native checksum mismatch".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut reader = BufReader::with_capacity(65536, file);
    let (mut game, memo): (PostFlopGame, String) =
        load_data_from_std_read(&mut reader, Some(request.max_memory_bytes))?;
    let expected_memo = request
        .input_sha256
        .as_ref()
        .map(|h| format!("riverline-river-tree/2:{h}"));
    if expected_memo
        .as_ref()
        .map_or(memo != "riverline-river-tree/2", |expected| {
            &memo != expected
        })
    {
        return Err("artifact-error: native model mismatch".into());
    }
    let node = query(&mut game, &request.path)?;
    let player = request.player;
    let hands = holes_to_strings(game.private_cards(player))?;
    let total = hands.len();
    if request.offset > total {
        return Err("unsupported-input: offset exceeds hand count".into());
    }
    let end = (request.offset + request.limit).min(total);
    let acting = node.player == Some(player);
    let rows = (request.offset..end)
        .map(|i| {
            let reachable =
                node.weights[player][i] > 0.0 && node.compatible_weights[player][i] > 0.0;
            Row {
                engine_index: i,
                combo: hands[i].clone(),
                own_path_weight: node.weights[player][i],
                compatible_weight: node.compatible_weights[player][i],
                equity: reachable.then_some(node.equity[player][i]),
                ev: reachable.then_some(node.ev[player][i]),
                strategy: (acting && reachable).then(|| {
                    (0..node.actions.len())
                        .map(|a| node.strategy[a * total + i])
                        .collect()
                }),
                action_ev: (acting && reachable).then(|| {
                    (0..node.actions.len())
                        .map(|a| node.action_ev[a * total + i])
                        .collect()
                }),
            }
        })
        .collect();
    Ok(NodePage {
        schema_version: 1,
        native_sha256: request.native_sha256.clone(),
        path: request.path.clone(),
        terminal: node.terminal,
        acting_player: node.player,
        player,
        actions: node.actions,
        invested: node.invested,
        unit: "centibb".into(),
        total,
        offset: request.offset,
        next_offset: (end < total).then_some(end),
        rows,
    })
}
