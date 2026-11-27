use riverline_solver::explicit::{build_explicit, run_explicit, ExplicitRiverInput};
use riverline_solver::{collect, run, RiverInput};
use std::io::Read;
use std::{env, fs, process};
mod worker;

fn read_bounded(path: &str, limit: usize) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!("input exceeds {limit} bytes"));
    }
    Ok(bytes)
}

fn execute() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 3 && args[1] == "worker-flop-page" {
        return worker::serve_model(std::path::Path::new(&args[2]), 5);
    }
    if args.len() == 3 && args[1] == "preflight-flop-page" {
        let input: riverline_solver::flop::FlopInput =
            serde_json::from_slice(&read_bounded(&args[2], 65536)?).map_err(|e| e.to_string())?;
        if input
            .ranges
            .iter()
            .any(|r| r.values().filter(|&&w| w > 0.0).count() > 16)
        {
            return Err("unsupported-input: flop/5 at most 16 combos per side".into());
        }
        let game = riverline_solver::flop::build_flop(&input)?;
        let memory = game.memory_usage().0;
        if memory > input.limits.max_memory_bytes {
            return Err("budget-exceeded: memory".into());
        }
        println!("{{\"ok\":true,\"estimatedBytes\":{memory}}}");
        return Ok(());
    }
    if args.len() == 4 && ["measure-flop", "preflight-flop"].contains(&args[1].as_str()) {
        use riverline_solver::flop::*;
        use riverline_solver::turn::{query_turn, Step};
        let bytes = read_bounded(&args[2], 1024 * 1024)?;
        let input: FlopInput = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if args[1] == "preflight-flop" {
            let game = build_flop(&input)?;
            let estimated = game.memory_usage().0;
            fs::write(&args[3], format!("{{\"estimatedBytes\":{estimated}}}"))
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        let (mut game, report) = run_flop(input)?;
        let root = query_turn(&mut game, &[])?;
        let mut selected = vec![root];
        if game.available_actions().len() > 1 {
            selected.push(query_turn(&mut game, &[Step::Action { index: 1 }])?);
        }
        let mut value = serde_json::to_value(report).map_err(|e| e.to_string())?;
        value["input"] = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        value["sampleNodes"] = serde_json::to_value(selected).map_err(|e| e.to_string())?;
        fs::write(
            &args[3],
            serde_json::to_vec(&value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if args.len() == 5 && args[1] == "query-turn-page" {
        let request: riverline_solver::turn_page::PageQuery =
            serde_json::from_slice(&read_bounded(&args[3], 16384)?).map_err(|e| e.to_string())?;
        let page =
            riverline_solver::turn_page::read_page(std::path::Path::new(&args[2]), &request)?;
        let bytes = serde_json::to_vec(&page).map_err(|e| e.to_string())?;
        if bytes.len() > 131072 {
            return Err("budget-exceeded: turn page >128KiB".into());
        }
        fs::write(&args[4], bytes).map_err(|e| e.to_string())?;
        return Ok(());
    }
    if args.len() == 4 && args[1] == "measure-turn" {
        use riverline_solver::turn::*;
        use sha2::{Digest, Sha256};
        let bytes = read_bounded(&args[2], 65536)?;
        let input: TurnInput = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let version = input.schema_version;
        let (mut game, report) = run_turn(input, || false, |_, _| {})?;
        let root = query_turn(&mut game, &[])?;
        let native = format!("{}.native.bin", &args[3]);
        postflop_solver::save_data_to_file(
            &game,
            &format!("riverline-turn/{version}:{:x}", Sha256::digest(&bytes)),
            &native,
            None,
        )?;
        let mut value = serde_json::to_value(report).map_err(|e| e.to_string())?;
        value["input"] = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        value["root"] = serde_json::to_value(root).map_err(|e| e.to_string())?;
        fs::write(
            &args[3],
            serde_json::to_vec(&value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!("{{\"ok\":true,\"model\":\"turn/{version}\"}}");
        return Ok(());
    }
    if args.len() == 5 && args[1] == "query-turn" {
        let request: riverline_solver::turn_reader::TurnQuery =
            serde_json::from_slice(&read_bounded(&args[3], 16384)?).map_err(|e| e.to_string())?;
        let node =
            riverline_solver::turn_reader::read_turn(std::path::Path::new(&args[2]), &request)?;
        let bytes = serde_json::to_vec(&node).map_err(|e| e.to_string())?;
        if bytes.len() > 131072 {
            return Err("budget-exceeded: turn node >128KiB".into());
        }
        fs::write(&args[4], bytes).map_err(|e| e.to_string())?;
        return Ok(());
    }
    if args.len() == 3 && ["preflight-turn", "preflight-turn-page"].contains(&args[1].as_str()) {
        let version = if args[1] == "preflight-turn" { 3 } else { 4 };
        let limit = if version == 3 { 16 } else { 128 };
        let input: riverline_solver::turn::TurnInput =
            serde_json::from_slice(&read_bounded(&args[2], 65536)?).map_err(|e| e.to_string())?;
        if input.schema_version != version
            || input
                .ranges
                .iter()
                .any(|r| r.values().filter(|&&w| w > 0.0).count() > limit)
        {
            return Err(format!(
                "unsupported-input: turn/{version} product limit {limit} combos per side"
            ));
        }
        let game = riverline_solver::turn::build_turn(&input)?;
        let memory = game.memory_usage().0;
        if memory > input.limits.max_memory_bytes {
            return Err("budget-exceeded: memory".into());
        }
        println!("{{\"ok\":true,\"estimatedBytes\":{memory}}}");
        return Ok(());
    }
    if args.len() == 3 && args[1] == "worker-turn" {
        return worker::serve_model(std::path::Path::new(&args[2]), 3);
    }
    if args.len() == 3 && args[1] == "worker-turn-page" {
        return worker::serve_model(std::path::Path::new(&args[2]), 4);
    }
    if args.len() == 4 && args[1] == "validate-turn" {
        use riverline_solver::turn::*;
        let input: TurnInput =
            serde_json::from_slice(&read_bounded(&args[2], 65536)?).map_err(|e| e.to_string())?;
        if input.ranges.iter().any(|r| r.len() > 8) {
            return Err("benchmark-only: at most 8 combos per side".into());
        }
        let (mut game, report) = run_turn(input, || false, |_, _| {})?;
        let nodes = collect_turn(&mut game)?;
        let archive = format!("{}.native.bin", &args[3]);
        postflop_solver::save_data_to_file(&game, "riverline-turn/3-benchmark", &archive, None)?;
        let (mut loaded, _): (postflop_solver::PostFlopGame, String) =
            postflop_solver::load_data_from_file(&archive, Some(256 * 1024 * 1024))?;
        if serde_json::to_value(query_turn(&mut loaded, &[])?).map_err(|e| e.to_string())?
            != serde_json::to_value(query_turn(&mut game, &[])?).map_err(|e| e.to_string())?
        {
            return Err("turn native root differs".into());
        }
        let mut value = serde_json::to_value(report).map_err(|e| e.to_string())?;
        // Preserve the exact declared input, rather than serde Value's f32-to-f64 expansion.
        value["input"] =
            serde_json::from_slice(&read_bounded(&args[2], 65536)?).map_err(|e| e.to_string())?;
        value["turnNodes"] = serde_json::to_value(nodes).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("benchmark-only: report >16MiB".into());
        }
        fs::write(&args[3], bytes).map_err(|e| e.to_string())?;
        println!("{{\"ok\":true,\"model\":\"turn/3\"}}");
        return Ok(());
    }
    if args.len() == 5 && args[1] == "query-river-tree" {
        let bytes = read_bounded(&args[3], 16384)?;
        let request: riverline_solver::native_reader::NativeQuery =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let page =
            riverline_solver::native_reader::read_page(std::path::Path::new(&args[2]), &request)?;
        let bytes = serde_json::to_vec(&page).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("budget-exceeded: node page >1MiB".into());
        }
        fs::write(&args[4], bytes).map_err(|e| e.to_string())?;
        return Ok(());
    }
    if args.len() == 3 && args[1] == "preflight-river" {
        let bytes = read_bounded(&args[2], 1024 * 1024)?;
        let input: RiverInput = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let game = riverline_solver::build(&input)?;
        let memory = game.memory_usage().0;
        if memory > input.limits.max_memory_bytes {
            return Err("budget-exceeded: memory".into());
        }
        println!("{{\"ok\":true,\"estimatedBytes\":{memory}}}");
        return Ok(());
    }
    if args.len() == 3 && args[1] == "worker" {
        return worker::serve(std::path::Path::new(&args[2]), false);
    }
    if args.len() == 3 && args[1] == "worker-tree" {
        return worker::serve(std::path::Path::new(&args[2]), true);
    }
    if args.len() == 3 && args[1] == "preflight-river-tree" {
        let bytes = read_bounded(&args[2], 1024 * 1024)?;
        let input: ExplicitRiverInput =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let game = build_explicit(&input)?;
        let memory = game.memory_usage().0;
        if memory > input.limits.max_memory_bytes {
            return Err("budget-exceeded: memory".into());
        }
        println!("{{\"ok\":true,\"estimatedBytes\":{memory}}}");
        return Ok(());
    }
    if args.len() == 4 && args[1] == "validate-river-tree" {
        let bytes = read_bounded(&args[2], 1024 * 1024)?;
        let input: ExplicitRiverInput =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let (mut game, mut report) = run_explicit(input, || false, |_, _| {})?;
        report.nodes = collect(&mut game)?;
        let archive = format!("{}.native.bin", args[3]);
        postflop_solver::save_data_to_file(&game, "riverline-river-tree/2", &archive, None)?;
        let (mut loaded, _): (postflop_solver::PostFlopGame, String) =
            postflop_solver::load_data_from_file(
                &archive,
                Some(report.input.limits.max_memory_bytes),
            )?;
        if serde_json::to_value(collect(&mut loaded)?).map_err(|e| e.to_string())?
            != serde_json::to_value(&report.nodes).map_err(|e| e.to_string())?
        {
            return Err("native-readback-mismatch".into());
        }
        fs::write(
            &args[3],
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{{\"ok\":true,\"iterations\":{},\"nodes\":{}}}",
            report.iterations,
            report.nodes.len()
        );
        return Ok(());
    }
    if args.len() != 4 || args[1] != "validate-river" {
        return Err("usage: riverline-solver validate-river INPUT.json OUTPUT.json".into());
    }
    let bytes = read_bounded(&args[2], 1024 * 1024)?;
    let input: RiverInput = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let (mut game, mut report) = run(input, || false, |_, _| {})?;
    report.nodes = collect(&mut game)?;
    // R1 verifies native serialization and readback without claiming resumable CFR state.
    let archive = format!("{}.native.bin", args[3]);
    postflop_solver::save_data_to_file(&game, "riverline-r1", &archive, None)?;
    let (mut restored, _): (postflop_solver::PostFlopGame, String) =
        postflop_solver::load_data_from_file(&archive, Some(report.input.limits.max_memory_bytes))?;
    let restored_nodes = collect(&mut restored)?;
    if serde_json::to_value(&restored_nodes).map_err(|e| e.to_string())?
        != serde_json::to_value(&report.nodes).map_err(|e| e.to_string())?
    {
        return Err("native-readback-mismatch".into());
    }
    fs::write(
        &args[3],
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "{{\"ok\":true,\"iterations\":{},\"exploitability\":{}}}",
        report.iterations, report.exploitability
    );
    Ok(())
}

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        process::exit(1);
    }
}
