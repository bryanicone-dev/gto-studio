//! Browser build of GTO Studio: the shared engine exposed to a Web Worker.
//! The worker drives the solve one small batch at a time (`step`), so it can report
//! progress and react to "stop" between batches. All results are JSON strings in the
//! same shape the native server's HTTP API returns.
//!
//! Library spots (`load`) only store the flop. When the user deals a turn, `node` answers
//! `resolve_needed` and the worker calls `resolve`, which solves that street as a subgame.

use gto_studio::engine::{self, SolveRequest};
use gto_studio::postflop_solver::{PostFlopGame, compute_exploitability, finalize, solve, solve_step};
use serde_json::{json, Value};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

struct Job {
    game: PostFlopGame,
    iteration: u32,
    max_iterations: u32,
    pot: f32,
    target: f32,
    exploitability_pct: f32,
    finished: bool,
    /// Turn/river subgame solved on demand: (history that leads to it, including the card, tree).
    sub: Option<(Vec<usize>, PostFlopGame)>,
}

thread_local! {
    static JOB: RefCell<Option<Job>> = const { RefCell::new(None) };
}

fn error(msg: impl Into<String>) -> String {
    json!({ "error": msg.into() }).to_string()
}

fn finished_job(game: PostFlopGame) -> Job {
    Job {
        game, iteration: 0, max_iterations: 0, pot: 0.0, target: 0.0,
        exploitability_pct: f32::NAN, finished: true, sub: None,
    }
}

/// Builds the tree and allocates its memory. Returns `{memory_gb, compressed}` or `{error}`.
#[wasm_bindgen]
pub fn prepare(request: &str, limit_gb: f64) -> String {
    JOB.with(|j| *j.borrow_mut() = None); // free the previous tree first
    let req: SolveRequest = match serde_json::from_str(request) {
        Ok(r) => r,
        Err(e) => return error(format!("JSON inválido: {e}")),
    };
    let (mut game, compress, memory_gb) = match engine::prepare(&req, limit_gb) {
        Ok(x) => x,
        Err(e) => return error(e),
    };
    game.allocate_memory(compress);
    let pot = game.tree_config().starting_pot as f32;
    JOB.with(|j| *j.borrow_mut() = Some(Job {
        iteration: 0, max_iterations: req.max_iterations, pot,
        target: pot * req.target_pct / 100.0, finished: false,
        ..finished_job(game)
    }));
    json!({ "memory_gb": memory_gb, "compressed": compress }).to_string()
}

/// Runs up to `count` iterations. Exploitability is measured every 10 iterations, like the
/// native server. Returns `{iteration, max_iterations, exploitability_pct, done}`.
#[wasm_bindgen]
pub fn step(count: u32) -> String {
    JOB.with(|j| {
        let mut j = j.borrow_mut();
        let Some(job) = j.as_mut().filter(|job| !job.finished) else {
            return error("Nenhum solve em andamento.");
        };
        let mut done = job.iteration >= job.max_iterations;
        for _ in 0..count {
            if done { break; }
            solve_step(&job.game, job.iteration);
            job.iteration += 1;
            if job.iteration % 10 == 0 || job.iteration == job.max_iterations {
                let e = compute_exploitability(&job.game);
                job.exploitability_pct = 100.0 * e / job.pot;
                done = e <= job.target || job.iteration == job.max_iterations;
            }
        }
        json!({
            "iteration": job.iteration, "max_iterations": job.max_iterations,
            "exploitability_pct": job.exploitability_pct, "done": done,
        }).to_string()
    })
}

/// Ends the solve (also after a stop) so the strategy can be browsed.
#[wasm_bindgen]
pub fn finish() {
    JOB.with(|j| {
        if let Some(job) = j.borrow_mut().as_mut().filter(|job| !job.finished) {
            finalize(&mut job.game);
            job.finished = true;
        }
    });
}

/// Opens a library spot (bytes of a file saved by the `library` tool). Returns `{memo}` or `{error}`.
#[wasm_bindgen]
pub fn load(bytes: &[u8]) -> String {
    JOB.with(|j| *j.borrow_mut() = None);
    match engine::load(bytes) {
        Ok((game, memo)) => {
            JOB.with(|j| *j.borrow_mut() = Some(finished_job(game)));
            json!({ "memo": memo }).to_string()
        }
        Err(e) => error(format!("Não foi possível abrir o spot: {e}")),
    }
}

/// Solves the turn/river subgame after `prefix` (a JSON array ending at a chance node) with
/// `card` dealt, keeping it for `node`. Returns `{memory_gb}` or `{error}`.
#[wasm_bindgen]
pub fn resolve(prefix: &str, card: usize, limit_gb: f64, max_iterations: u32, target_pct: f32) -> String {
    let Ok(prefix) = serde_json::from_str::<Vec<usize>>(prefix) else {
        return error("Esperado um histórico como [0, 1, ...].");
    };
    JOB.with(|j| {
        let mut j = j.borrow_mut();
        let Some(job) = j.as_mut().filter(|job| job.finished) else {
            return error("Nenhuma solução pronta.");
        };
        job.sub = None; // free the previous subgame first
        let mut sub = match engine::subgame_at_chance(&mut job.game, &prefix, card) {
            Ok(g) => g,
            Err(e) => return error(e),
        };
        let (raw, comp) = sub.memory_usage();
        let (compress, memory_gb) = match engine::choose_precision(raw as f64 / engine::GB, comp as f64 / engine::GB, "auto", limit_gb) {
            Ok(x) => x,
            Err(e) => return error(e),
        };
        sub.allocate_memory(compress);
        let target = sub.tree_config().starting_pot as f32 * target_pct / 100.0;
        solve(&mut sub, max_iterations, target, false);
        let mut key = prefix;
        key.push(card);
        job.sub = Some((key, sub));
        json!({ "memory_gb": memory_gb }).to_string()
    })
}

/// JSON of the node reached by `history` (a JSON array of indices), or `{error}`.
/// Answers `{type: "resolve_needed", prefix, card}` when that street must be solved first.
#[wasm_bindgen]
pub fn node(history: &str) -> String {
    let Ok(history) = serde_json::from_str::<Vec<usize>>(history) else {
        return error("Esperado um histórico como [0, 1, ...].");
    };
    JOB.with(|j| match j.borrow_mut().as_mut().filter(|job| job.finished) {
        None => error("Nenhuma solução pronta. Rode um solve primeiro."),
        Some(job) => match node_in_job(job, &history) {
            Ok(v) => v.to_string(),
            Err(e) => error(e),
        },
    })
}

fn node_in_job(job: &mut Job, history: &[usize]) -> Result<Value, String> {
    let Some((key, sub)) = job.sub.as_mut().filter(|(key, _)| history.starts_with(key)) else {
        return engine::node_json(&mut job.game, history);
    };
    // Inside the subgame: its own path, prefixed by the parent's path up to the dealt card.
    let parent = engine::node_json(&mut job.game, &key[..key.len() - 1])?;
    let mut v = engine::node_json(sub, &history[key.len()..])?;
    let card = engine::card_str(*key.last().unwrap() as u8);
    let mut path = parent["path"].as_array().cloned().unwrap_or_default();
    path.push(json!({ "card": card }));
    path.extend(v["path"].as_array().cloned().unwrap_or_default());
    v["path"] = Value::Array(path);
    v["history"] = json!(history);
    Ok(v)
}
