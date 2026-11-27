use crate::turn::{query_turn, Step, TurnNode};
use postflop_solver::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnQuery {
    pub schema_version: u32,
    pub native_sha256: String,
    pub input_sha256: String,
    pub path: Vec<Step>,
    pub max_memory_bytes: u64,
}
pub fn read_turn(path: &Path, request: &TurnQuery) -> Result<TurnNode, String> {
    let mut game = load_turn(path, request, 3, 16)?;
    query_turn(&mut game, &request.path)
}
pub fn load_turn(
    path: &Path,
    request: &TurnQuery,
    version: u32,
    limit: usize,
) -> Result<PostFlopGame, String> {
    let valid_hash = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    };
    if request.schema_version != version
        || request.path.len() > 24
        || !valid_hash(&request.native_sha256)
        || !valid_hash(&request.input_sha256)
        || request.max_memory_bytes == 0
        || request.max_memory_bytes > 256 * 1024 * 1024
    {
        return Err("unsupported-input: invalid turn query".into());
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.file_type().is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err("artifact-error: invalid turn native file".into());
    }
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
    let (game, memo): (PostFlopGame, String) =
        load_data_from_std_read(&mut reader, Some(request.max_memory_bytes))?;
    if memo != format!("riverline-turn/{version}:{}", request.input_sha256) {
        return Err("artifact-error: turn model/input mismatch".into());
    }
    if [0, 1].iter().any(|&p| game.private_cards(p).len() > limit) {
        return Err(format!(
            "unsupported-input: turn product limit {limit} combos per side"
        ));
    }
    Ok(game)
}
