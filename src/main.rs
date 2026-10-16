//! GTO Studio native server: serves the UI and runs the solver on this machine.
//!
//! Environment:
//!   GTO_HOST            bind address (default 127.0.0.1; use 0.0.0.0 inside a container)
//!   GTO_PORT            HTTP port (default 7878)
//!   GTO_MAX_MEMORY_GB   memory cap for a tree (default: 85% of RAM available at solve time)
//!   RAYON_NUM_THREADS   solver threads (default: all logical cores)

use gto_studio::engine::{self, GB, SolveRequest};
use gto_studio::postflop_solver::{PostFlopGame, compute_exploitability, finalize, solve_step};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tiny_http::{Header, Method, Response, Server};

#[derive(Default)]
struct Status {
    state: &'static str, // idle | solving | done
    iteration: u32,
    max_iterations: u32,
    exploitability_pct: f32,
    memory_gb: f64,
    compressed: bool,
    cancelled: bool,
    error: String,
}

type GameSlot = Arc<Mutex<Option<PostFlopGame>>>;
type StatusSlot = Arc<Mutex<Status>>;

fn main() {
    let host = std::env::var("GTO_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("GTO_PORT").unwrap_or_else(|_| "7878".into());
    let addr = format!("{host}:{port}");
    let server = Server::http(&addr).unwrap_or_else(|e| panic!("não foi possível abrir {addr}: {e}"));
    println!("GTO Studio em http://{addr}  (threads: {})", rayon_threads());

    let game: GameSlot = Arc::default();
    let status: StatusSlot = Arc::new(Mutex::new(Status { state: "idle", ..Default::default() }));
    let cancel = Arc::new(AtomicBool::new(false));

    for mut req in server.incoming_requests() {
        let mut body = String::new();
        let _ = req.as_reader().read_to_string(&mut body);
        let (code, payload) = match (req.method(), req.url()) {
            (Method::Get, "/") => {
                let html = Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
                let _ = req.respond(Response::from_string(include_str!("../static/index.html")).with_header(html));
                continue;
            }
            (Method::Post, "/api/solve") => start_solve(&body, &game, &status, &cancel),
            (Method::Post, "/api/cancel") => {
                cancel.store(true, Ordering::Relaxed);
                (200, json!({}))
            }
            (Method::Get, "/api/status") => (200, status_json(&status.lock().unwrap())),
            (Method::Post, "/api/node") => node(&body, &game),
            _ => (404, json!({ "error": "not found" })),
        };
        let ct = Header::from_bytes("Content-Type", "application/json").unwrap();
        let _ = req.respond(Response::from_string(payload.to_string()).with_status_code(code).with_header(ct));
    }
}

fn rayon_threads() -> usize {
    std::env::var("RAYON_NUM_THREADS").ok().and_then(|v| v.parse().ok())
        .unwrap_or_else(|| thread::available_parallelism().map_or(1, |n| n.get()))
}

/// (memory cap in GB, total RAM in GB). The cap follows RAM that is free right now.
fn memory_limit() -> (f64, f64) {
    let (total, available) = system_memory();
    let cap = std::env::var("GTO_MAX_MEMORY_GB").ok().and_then(|v| v.parse().ok())
        .unwrap_or(available as f64 / GB * 0.85);
    (cap, total as f64 / GB)
}

/// (total, available) bytes of physical memory.
#[cfg(windows)]
fn system_memory() -> (u64, u64) {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32, memory_load: u32, total_phys: u64, avail_phys: u64,
        total_page_file: u64, avail_page_file: u64, total_virtual: u64, avail_virtual: u64,
        avail_extended_virtual: u64,
    }
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }
    let mut m: MemoryStatusEx = unsafe { std::mem::zeroed() };
    m.length = std::mem::size_of::<MemoryStatusEx>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 { return (0, 0); }
    (m.total_phys, m.avail_phys)
}

#[cfg(target_os = "linux")]
fn system_memory() -> (u64, u64) {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let field = |name: &str| info.lines().find(|l| l.starts_with(name))
        .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) * 1024;
    let (mut total, mut available) = (field("MemTotal:"), field("MemAvailable:"));
    // Inside a container the cgroup v2 limit is the real ceiling, not the host's RAM.
    let read = |path| std::fs::read_to_string(path).ok().and_then(|s| s.trim().parse::<u64>().ok());
    if let Some(max) = read("/sys/fs/cgroup/memory.max") {
        let used = read("/sys/fs/cgroup/memory.current").unwrap_or(0);
        total = total.min(max);
        available = available.min(max.saturating_sub(used));
    }
    (total, available)
}

// ponytail: macOS exposes no simple "available" figure; assume 60% of total is usable.
#[cfg(not(any(windows, target_os = "linux")))]
fn system_memory() -> (u64, u64) {
    let total = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok()?.trim().parse::<u64>().ok()).unwrap_or(0);
    (total, total / 10 * 6)
}

fn status_json(s: &Status) -> Value {
    let (limit_gb, total_gb) = memory_limit();
    json!({
        "state": s.state, "iteration": s.iteration, "max_iterations": s.max_iterations,
        "exploitability_pct": s.exploitability_pct, "memory_gb": s.memory_gb,
        "compressed": s.compressed, "cancelled": s.cancelled, "error": s.error,
        "limit_gb": limit_gb, "total_ram_gb": total_gb, "threads": rayon_threads(),
    })
}

fn start_solve(body: &str, game: &GameSlot, status: &StatusSlot, cancel: &Arc<AtomicBool>) -> (u16, Value) {
    if status.lock().unwrap().state == "solving" {
        return (409, json!({ "error": "Já existe um solve em andamento." }));
    }
    let req: SolveRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return (400, json!({ "error": format!("JSON inválido: {e}") })),
    };
    // Free the previous tree before measuring available memory.
    *game.lock().unwrap() = None;
    status.lock().unwrap().state = "idle";
    let (mut g, compress, memory_gb) = match engine::prepare(&req, memory_limit().0) {
        Ok(x) => x,
        Err(e) => return (400, json!({ "error": e })),
    };

    cancel.store(false, Ordering::Relaxed);
    *status.lock().unwrap() = Status {
        state: "solving", max_iterations: req.max_iterations, memory_gb, compressed: compress,
        exploitability_pct: f32::NAN, ..Default::default()
    };

    let (game, status, cancel) = (game.clone(), status.clone(), cancel.clone());
    thread::spawn(move || {
        g.allocate_memory(compress);
        let pot = g.tree_config().starting_pot as f32;
        let target = pot * req.target_pct / 100.0;
        for i in 0..req.max_iterations {
            if cancel.load(Ordering::Relaxed) {
                status.lock().unwrap().cancelled = true;
                break;
            }
            solve_step(&g, i);
            let check = (i + 1) % 10 == 0 || i + 1 == req.max_iterations;
            let e = if check { Some(compute_exploitability(&g)) } else { None };
            let mut s = status.lock().unwrap();
            s.iteration = i + 1;
            if let Some(e) = e {
                s.exploitability_pct = 100.0 * e / pot;
                if e <= target { break; }
            }
        }
        // A cancelled solve is still finalized, so its current strategy can be browsed.
        finalize(&mut g);
        *game.lock().unwrap() = Some(g);
        status.lock().unwrap().state = "done";
    });
    (202, json!({ "memory_gb": memory_gb, "compressed": compress }))
}

fn node(body: &str, slot: &GameSlot) -> (u16, Value) {
    let history: Vec<usize> = match serde_json::from_str::<Value>(body)
        .ok().and_then(|v| serde_json::from_value(v["history"].clone()).ok())
    {
        Some(h) => h,
        None => return (400, json!({ "error": "Esperado {\"history\": [..]}" })),
    };
    let mut lock = slot.lock().unwrap();
    let Some(g) = lock.as_mut() else {
        return (409, json!({ "error": "Nenhuma solução pronta. Rode um solve primeiro." }));
    };
    match engine::node_json(g, &history) {
        Ok(v) => (200, v),
        Err(e) => (400, json!({ "error": e })),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_system_memory() {
        let (total, available) = super::system_memory();
        assert!(total > 0 && available > 0 && available <= total, "{total} {available}");
    }
}
