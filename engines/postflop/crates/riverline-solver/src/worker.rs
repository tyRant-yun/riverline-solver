//! Bounded process protocol. Only the parent publishes business artifacts.
use riverline_solver::explicit::{run_explicit, ExplicitRiverInput};
use riverline_solver::{query, run, RiverInput, ENGINE_COMMIT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const MAX_FRAME: u64 = 262144;
const PROTOCOL: &str = "riverline-engine/1";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Identity {
    request_id: String,
    job_id: String,
    attempt: u32,
    spec_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Command {
    protocol_version: String,
    operation: String,
    identity: Identity,
}

fn read_command(reader: &mut impl BufRead) -> Result<Option<Command>, String> {
    let mut bytes = vec![];
    let read = reader
        .take(MAX_FRAME + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if read == 0 {
        return Ok(None);
    }
    if read as u64 > MAX_FRAME || bytes.last() != Some(&b'\n') {
        return Err("protocol-error: overlong/incomplete frame".into());
    }
    let command: Command =
        serde_json::from_slice(&bytes).map_err(|e| format!("protocol-error: {e}"))?;
    if command.protocol_version != PROTOCOL
        || command.identity.attempt == 0
        || command.identity.spec_hash.len() != 64
        || !command
            .identity
            .spec_hash
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
        || [&command.identity.job_id, &command.identity.request_id]
            .iter()
            .any(|s| {
                s.is_empty()
                    || s.len() > 128
                    || !s
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            })
    {
        return Err("protocol-error: invalid identity or version".into());
    }
    Ok(Some(command))
}

fn emit(
    kind: &str,
    identity: Option<&Identity>,
    seq: u32,
    payload: serde_json::Value,
) -> Result<(), String> {
    let event = serde_json::json!({"protocolVersion": PROTOCOL, "kind":kind, "identity":identity, "seq":seq, "payload":payload});
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, &event).map_err(|e| e.to_string())?;
    output.write_all(b"\n").map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())
}

pub fn serve(directory: &Path, explicit: bool) -> Result<(), String> {
    serve_model(directory, if explicit { 2 } else { 1 })
}
pub fn serve_model(directory: &Path, model: u8) -> Result<(), String> {
    let explicit = model == 2;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    let input_path = directory.join("input.json");
    let native_path = directory.join("solution.bin");
    let summary_path = directory.join("summary.json");
    let metadata = fs::symlink_metadata(&input_path).map_err(|e| e.to_string())?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 1024 * 1024
        || fs::symlink_metadata(&native_path).is_ok()
        || fs::symlink_metadata(&summary_path).is_ok()
    {
        return Err("artifact-error: unsafe input or existing output".into());
    }
    emit(
        "hello",
        None,
        0,
        serde_json::json!({"engineCommit":ENGINE_COMMIT,"algorithmId":"upstream-dcfr-gamma3-v1","features":["bincode"],"resume":false}),
    )?;
    let stdin = io::stdin();
    let first = read_command(&mut stdin.lock())?.ok_or("protocol-error: missing start")?;
    if first.operation != "solve" {
        return Err("protocol-error: first command must solve".into());
    }
    let bytes = fs::read(input_path).map_err(|e| e.to_string())?;
    if format!("{:x}", Sha256::digest(&bytes)) != first.identity.spec_hash {
        return Err("artifact-error: input hash mismatch".into());
    }

    let cancelled = Arc::new(AtomicBool::new(false));
    let invalid = Arc::new(AtomicBool::new(false));
    let reader_cancelled = Arc::clone(&cancelled);
    let reader_invalid = Arc::clone(&invalid);
    let identity = first.identity;
    let reader_identity = identity.clone();
    std::thread::spawn(move || {
        let mut reader = io::stdin().lock();
        match read_command(&mut reader) {
            Ok(Some(command))
                if command.operation == "cancel" && command.identity == reader_identity =>
            {
                reader_cancelled.store(true, Ordering::SeqCst);
            }
            Ok(None) => {
                reader_cancelled.store(true, Ordering::SeqCst);
            }
            _ => {
                reader_invalid.store(true, Ordering::SeqCst);
                reader_cancelled.store(true, Ordering::SeqCst);
            }
        }
    });
    let mut seq = 1;
    emit("started", Some(&identity), seq, serde_json::json!({}))?;
    seq += 1;
    let progress = |iterations, exploitability| {
        if iterations % 1000 == 0 {
            if emit("progress", Some(&identity), seq, serde_json::json!({"iterations":iterations,"exploitability":exploitability,"unit":"centibb"})).is_err() { cancelled.store(true, Ordering::SeqCst); }
            seq += 1;
        }
    };
    let result = if model == 5 {
        let input: riverline_solver::flop::FlopInput =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if input.schema_version != 5
            || input
                .ranges
                .iter()
                .any(|r| r.values().filter(|&&w| w > 0.0).count() > 16)
            || input.limits.max_memory_bytes > 256 * 1024 * 1024
            || input.limits.max_seconds > 120
        {
            return Err("unsupported-input: flop/5 product limit".into());
        }
        riverline_solver::flop::run_flop_controlled(
            input,
            || cancelled.load(Ordering::SeqCst),
            progress,
        )
        .and_then(|(g, r)| Ok((g, serde_json::to_value(r).map_err(|e| e.to_string())?)))
    } else if model >= 3 {
        let input: riverline_solver::turn::TurnInput =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if input.schema_version != model as u32
            || input.ranges.iter().any(|r| {
                r.values().filter(|&&w| w > 0.0).count() > if model == 3 { 16 } else { 128 }
            })
        {
            return Err("unsupported-input: turn product range/schema limit".into());
        }
        riverline_solver::turn::run_turn(input, || cancelled.load(Ordering::SeqCst), progress)
            .and_then(|(g, r)| Ok((g, serde_json::to_value(r).map_err(|e| e.to_string())?)))
    } else if explicit {
        let input: ExplicitRiverInput =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        run_explicit(input, || cancelled.load(Ordering::SeqCst), progress)
            .and_then(|(g, r)| Ok((g, serde_json::to_value(r).map_err(|e| e.to_string())?)))
    } else {
        let input: RiverInput = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        run(input, || cancelled.load(Ordering::SeqCst), progress)
            .and_then(|(g, r)| Ok((g, serde_json::to_value(r).map_err(|e| e.to_string())?)))
    };
    if invalid.load(Ordering::SeqCst) {
        emit(
            "failed",
            Some(&identity),
            seq,
            serde_json::json!({"code":"protocol-error"}),
        )?;
        return Ok(());
    }
    match result {
        Err(message) => emit(
            if message == "cancelled" {
                "cancelled"
            } else {
                "failed"
            },
            Some(&identity),
            seq,
            serde_json::json!({"message":message}),
        ),
        Ok((game, report)) => {
            if cancelled.load(Ordering::SeqCst) {
                return emit("cancelled", Some(&identity), seq, serde_json::json!({}));
            }
            let memo = if model >= 3 {
                format!("riverline-turn/{model}:{}", identity.spec_hash)
            } else if explicit {
                format!("riverline-river-tree/2:{}", identity.spec_hash)
            } else {
                "riverline-engine/1".into()
            };
            postflop_solver::save_data_to_file(&game, &memo, &native_path, None)?;
            let (mut loaded, _): (postflop_solver::PostFlopGame, String) =
                postflop_solver::load_data_from_file(&native_path, Some(2 * 1024 * 1024 * 1024))?;
            // This supported model has exactly six nodes. Keep each projection separate;
            // a general/multistreet tree must use a separate on-demand native reader.
            let paths = [vec![], vec![0], vec![0, 0], vec![1], vec![1, 0], vec![1, 1]];
            let mut pages = vec![];
            if model >= 3 {
                riverline_solver::turn::query_turn(&mut loaded, &[])?;
            } else {
                query(&mut loaded, &[])?;
            }
            for (index, path) in paths.iter().enumerate().filter(|_| model == 1) {
                let node = query(&mut loaded, path)?;
                let filename = format!("node-{index}.json");
                let output = directory.join(&filename);
                if fs::symlink_metadata(&output).is_ok() {
                    return Err("artifact-error: node output exists".into());
                }
                let bytes = serde_json::to_vec(&node).map_err(|e| e.to_string())?;
                fs::write(output, &bytes).map_err(|e| e.to_string())?;
                pages.push(serde_json::json!({"path":path,"file":filename,"hash":format!("{:x}",Sha256::digest(&bytes)),"bytes":bytes.len()}));
            }
            let mut summary = report;
            if model >= 3 {
                summary["input"] = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            }
            summary["nodes"] = pages.into();
            summary["identity"] = serde_json::to_value(&identity).map_err(|e| e.to_string())?;
            fs::write(
                &summary_path,
                serde_json::to_vec(&summary).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            // Cancellation arriving after publication staging is resolved by the Node transaction.
            emit(
                "completed",
                Some(&identity),
                seq,
                serde_json::json!({"nativeFile":"solution.bin","summaryFile":"summary.json"}),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_golden_and_malformed_frames() {
        let golden = include_str!("../../../../../contracts/solver/examples/solve.json");
        let mut value: serde_json::Value = serde_json::from_str(golden).unwrap();
        let line = serde_json::to_string(&value).unwrap() + "\n";
        let command = read_command(&mut line.as_bytes()).unwrap().unwrap();
        assert_eq!(command.operation, "solve");
        value["extra"] = true.into();
        assert!(
            read_command(&mut (serde_json::to_string(&value).unwrap() + "\n").as_bytes()).is_err()
        );
        assert!(read_command(&mut b"{}".as_slice()).is_err());
        assert!(read_command(&mut vec![b' '; MAX_FRAME as usize + 1].as_slice()).is_err());
    }
}
