//! Builds the spot library shipped with the browser version: solves each preset x board on
//! this machine and saves only the flop decisions (turn and river are re-solved in the browser).
//!
//! Usage: cargo run --release --bin library -- deploy/library/spots.json library ["preset name"]
//! With a preset name only that preset is solved (CI runs one preset per machine).
//! Spots already present in the output folder are skipped, so the build can be resumed.

use gto_studio::engine::{self, SolveRequest, StreetSizes};
use gto_studio::postflop_solver::{compute_exploitability, finalize, solve_step};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Instant;
use std::{env, fs};

#[derive(Deserialize)]
struct Spec {
    bet: String,
    raise: String,
    target_pct: f32,
    max_iterations: u32,
    presets: Vec<String>,
    boards: Vec<String>,
}

/// (pot, stack, oop range, ip range) of a preset, read from the UI so there is one source of truth.
fn preset(html: &str, name: &str) -> Option<(f64, f64, String, String)> {
    let start = html.find(&format!("name: '{name}'"))?;
    let block = &html[start..start + html[start..].find('}')?];
    let field = |key: &str, end: char| {
        let s = &block[block.find(key)? + key.len()..];
        Some(s[..s.find(end)?].trim().to_string())
    };
    Some((field("pot:", ',')?.parse().ok()?, field("stack:", ',')?.parse().ok()?, field("oop: '", '\'')?, field("ip: '", '\'')?))
}

fn slug(s: &str) -> String {
    s.chars().filter_map(|c| match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' => Some(c.to_ascii_lowercase()),
        ' ' | '-' | ',' | '.' => Some('-'),
        _ => None,
    }).collect::<String>().split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let (spec_path, out, only) = match args.as_slice() {
        [_, spec, out] => (spec.clone(), out.clone(), None),
        [_, spec, out, preset] => (spec.clone(), out.clone(), Some(preset.clone())),
        _ => return Err("uso: library <spots.json> <pasta-de-saida> [preset]".into()),
    };
    let spec: Spec = serde_json::from_str(&fs::read_to_string(&spec_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("spots.json inválido: {e}"))?;
    let html = include_str!("../../static/index.html");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let index_path = format!("{out}/index.json");
    let mut index: Vec<Value> = fs::read_to_string(&index_path).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    if let Some(p) = &only {
        if !spec.presets.contains(p) { return Err(format!("preset não está em spots.json: {p}")); }
    }
    for name in spec.presets.iter().filter(|n| only.as_ref().is_none_or(|p| p == *n)) {
        let (pot, stack, oop, ip) = preset(html, name).ok_or(format!("preset não encontrado: {name}"))?;
        for board in &spec.boards {
            let id = slug(&format!("{name} {board}"));
            let file = format!("{id}.bin");
            if index.iter().any(|s| s["id"] == id.as_str()) && fs::metadata(format!("{out}/{file}")).is_ok() {
                println!("já existe: {id}");
                continue;
            }
            let sizes = || StreetSizes { oop_bet: spec.bet.clone(), oop_raise: spec.raise.clone(), ip_bet: spec.bet.clone(), ip_raise: spec.raise.clone() };
            let req = SolveRequest {
                oop_range: oop.clone(), ip_range: ip.clone(), board: board.clone(), pot, stack,
                flop: sizes(), turn: sizes(), river: sizes(), turn_donk: String::new(), river_donk: String::new(),
                rake_pct: 0.0, rake_cap: 0.0, add_allin_threshold: 1.5, force_allin_threshold: 0.15,
                merging_threshold: 0.1, precision: "compressed".into(),
                max_iterations: spec.max_iterations, target_pct: spec.target_pct,
            };
            let (mut g, compress, memory_gb) = engine::prepare(&req, f64::MAX)?;
            println!("resolvendo {id} ({memory_gb:.2} GB)…");
            let started = Instant::now();
            g.allocate_memory(compress);
            let target = pot as f32 * 100.0 * spec.target_pct / 100.0;
            let (mut iterations, mut expl) = (0, f32::NAN);
            for i in 0..spec.max_iterations {
                solve_step(&g, i);
                iterations = i + 1;
                if iterations % 10 == 0 {
                    let e = compute_exploitability(&g);
                    expl = 100.0 * e / (pot as f32 * 100.0);
                    println!("  iteração {iterations}: exploitability {expl:.2}% do pote");
                    if e <= target { break; }
                }
            }
            finalize(&mut g);
            let bytes = engine::save_flop(&mut g, &id)?;
            fs::write(format!("{out}/{file}"), &bytes).map_err(|e| e.to_string())?;
            index.retain(|s| s["id"] != id.as_str());
            index.push(json!({
                "id": id, "preset": name, "board": board, "file": file, "bytes": bytes.len(),
                "iterations": iterations, "exploitability_pct": expl, "bet": spec.bet, "raise": spec.raise,
                "solve_seconds": started.elapsed().as_secs(),
            }));
            fs::write(&index_path, serde_json::to_string_pretty(&index).unwrap()).map_err(|e| e.to_string())?;
            println!("  salvo: {file} ({:.1} MB, {}s)", bytes.len() as f64 / 1e6, started.elapsed().as_secs());
        }
    }
    Ok(())
}
