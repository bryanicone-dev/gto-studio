//! Measures how much the site's "flop parecido" costs: for every library spot, pretends it is missing,
//! takes the nearest other flop of the same preset (same distance as the site's flopDistance) and plays
//! the exact spot with that flop's per-hand strategy. Compares OOP's first decision and IP's answer to a check.
//!
//! Usage: cargo run --release --bin compare -- <library folder with index.json> > relatorio.md
//! Per hand class (13x13 cell): whether the most used action is the same, and the EV lost
//! (exact EV minus the exact action EVs weighted by the approximate frequencies), in % of the pot.

use gto_studio::engine;
use serde_json::Value;
use std::collections::BTreeMap;
use std::{env, fs};

const RANKS: &str = "23456789TJQKA";

fn ranks_suits(board: &str) -> (Vec<usize>, usize) {
    let cards: Vec<&str> = (0..3).map(|i| &board[2 * i..2 * i + 2]).collect();
    let mut r: Vec<usize> = cards.iter().map(|c| RANKS.find(&c[..1]).unwrap()).collect();
    r.sort_by(|a, b| b.cmp(a));
    let mut s: Vec<&str> = cards.iter().map(|c| &c[1..]).collect();
    s.sort();
    s.dedup();
    (r, s.len())
}

/// Same as flopDistance in static/index.html.
fn distance(a: &str, b: &str) -> usize {
    let ((ra, sa), (rb, sb)) = (ranks_suits(a), ranks_suits(b));
    let paired = |r: &[usize]| r[0] == r[1] || r[1] == r[2];
    2 * ra[0].abs_diff(rb[0]) + ra[1].abs_diff(rb[1]) + ra[2].abs_diff(rb[2])
        + 6 * usize::from(sa != sb) + 10 * usize::from(paired(&ra) != paired(&rb))
}

fn texture(board: &str) -> &'static str {
    let (r, s) = ranks_suits(board);
    if r[0] == r[1] || r[1] == r[2] { "pareado" } else if s == 1 { "monotone" } else if s == 2 { "dois naipes" } else { "rainbow" }
}

/// OOP's first decision and IP's answer to a check.
fn nodes(dir: &str, file: &str) -> Result<[Value; 2], String> {
    let (mut g, _) = engine::load(&fs::read(format!("{dir}/{file}")).map_err(|e| e.to_string())?)?;
    let root = engine::node_json(&mut g, &[])?;
    let check = root["actions"].as_array().and_then(|a| a.iter().position(|x| x["kind"] == "Check")).ok_or("sem check na raiz")?;
    Ok([root, engine::node_json(&mut g, &[check])?])
}

#[derive(Default, Clone, Copy)]
struct Acc { w: f64, same: f64, loss: f64, n: usize, dist: f64 }

impl Acc {
    fn add(&mut self, o: &Acc) { self.w += o.w; self.same += o.same; self.loss += o.loss; self.n += o.n; self.dist += o.dist; }
    fn row(&self, name: &str) -> String {
        format!("| {name} | {} | {:.1}% | {:.2}% | {:.1} |", self.n, 100.0 * self.same / self.w, 100.0 * self.loss / self.w, self.dist / self.n as f64)
    }
}

fn floats(v: &Value) -> Vec<f64> { v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default() }

/// Compares one decision: weights are the exact spot's combos per cell.
fn compare(exact: &Value, approx: &Value) -> Option<Acc> {
    if exact["actions"] != approx["actions"] { return None; }
    let pot = exact["pot"].as_f64()?;
    let mut acc = Acc::default();
    for (e, a) in exact["grid"].as_array()?.iter().zip(approx["grid"].as_array()?) {
        let (w, fe, fa, evs) = (e["weight"].as_f64()?, floats(&e["freqs"]), floats(&a["freqs"]), floats(&e["evs"]));
        if w <= 0.0 || a["weight"].as_f64()? <= 0.0 || fe.len() != fa.len() || evs.len() != fe.len() { continue; }
        let best = |f: &[f64]| (0..f.len()).max_by(|&i, &j| f[i].total_cmp(&f[j])).unwrap();
        let played: f64 = fa.iter().zip(&evs).map(|(f, ev)| f * ev).sum();
        let ev: f64 = fe.iter().zip(&evs).map(|(f, ev)| f * ev).sum();
        acc.w += w;
        acc.same += w * f64::from(u8::from(best(&fe) == best(&fa)));
        acc.loss += w * (ev - played).max(0.0) / pot;
    }
    (acc.w > 0.0).then_some(acc)
}

fn main() -> Result<(), String> {
    let dir = env::args().nth(1).ok_or("uso: compare <pasta-da-biblioteca>")?;
    let index: Vec<Value> = serde_json::from_str(&fs::read_to_string(format!("{dir}/index.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut presets: Vec<&str> = vec![];
    for s in &index { let p = s["preset"].as_str().unwrap_or(""); if !presets.contains(&p) { presets.push(p); } }

    println!("# Flop parecido vs flop exato\n");
    println!("Cada flop da biblioteca é tratado como se faltasse e jogado com a estratégia do flop mais parecido da mesma situação.");
    println!("**Mesma melhor jogada**: % das mãos (pesadas por combos) em que a ação mais usada é a mesma. **EV perdido**: média, em % do pote.\n");
    let (mut by_texture, mut total): (BTreeMap<&str, [Acc; 2]>, [Acc; 2]) = (BTreeMap::new(), [Acc::default(); 2]);
    for preset in presets {
        let spots: Vec<&Value> = index.iter().filter(|s| s["preset"] == preset).collect();
        if spots.len() < 2 { continue; }
        let mut cache: Vec<Option<[Value; 2]>> = spots.iter().map(|s| nodes(&dir, s["file"].as_str().unwrap_or("")).map_err(|e| eprintln!("{}: {e}", s["id"])).ok()).collect();
        let mut sum = [Acc::default(); 2];
        for i in 0..spots.len() {
            let board = spots[i]["board"].as_str().unwrap_or("");
            let Some(j) = (0..spots.len()).filter(|&j| j != i && cache[j].is_some())
                .min_by_key(|&j| distance(board, spots[j]["board"].as_str().unwrap_or(""))) else { continue };
            let (Some(e), Some(a)) = (cache[i].take(), cache[j].as_ref()) else { continue };
            for k in 0..2 {
                if let Some(mut acc) = compare(&e[k], &a[k]) {
                    acc.n = 1;
                    acc.dist = distance(board, spots[j]["board"].as_str().unwrap_or("")) as f64;
                    let (w, same, loss) = (acc.w, acc.same, acc.loss);
                    // Each flop counts the same, whatever the number of combos.
                    acc = Acc { w: 1.0, same: same / w, loss: loss / w, ..acc };
                    sum[k].add(&acc);
                    by_texture.entry(texture(board)).or_default()[k].add(&acc);
                }
            }
            cache[i] = Some(e);
        }
        println!("## {preset} ({} flops)\n\n| Decisão | Flops | Mesma melhor jogada | EV perdido | Distância média |\n|---|---|---|---|---|", spots.len());
        println!("{}\n{}\n", sum[0].row("OOP, 1ª ação"), sum[1].row("IP após check"));
        total[0].add(&sum[0]);
        total[1].add(&sum[1]);
    }
    println!("## Por textura (todas as situações)\n\n| Textura | Decisão | Flops | Mesma melhor jogada | EV perdido | Distância média |\n|---|---|---|---|---|---|");
    for (t, acc) in &by_texture {
        println!("{}\n{}", acc[0].row(&format!("{t} | OOP")), acc[1].row(&format!("{t} | IP após check")));
    }
    println!("\n**Total:** OOP {:.1}% mesma jogada, {:.2}% do pote perdido · IP {:.1}%, {:.2}%",
        100.0 * total[0].same / total[0].w, 100.0 * total[0].loss / total[0].w, 100.0 * total[1].same / total[1].w, 100.0 * total[1].loss / total[1].w);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_matches_site() {
        assert_eq!(distance("Ks7d2c", "Ks6d2c"), 1);
        assert_eq!(distance("Ks7d2c", "Ks7s2s"), 6);
        assert_eq!(distance("Ks7d2c", "KsKd2c"), 2 * 0 + 6 + 0 + 10);
        assert_eq!(texture("9h8h7c"), "dois naipes");
        assert_eq!(texture("JhJd5c"), "pareado");
    }
}
