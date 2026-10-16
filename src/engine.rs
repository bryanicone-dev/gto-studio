//! Solver engine shared by the native server and the browser (WebAssembly) build:
//! builds and solves postflop trees and turns any tree node into the JSON the UI renders.
//! Amounts are stored in centi-bb (1 bb = 100 chips) because the solver uses integers.

use crate::hands;
use postflop_solver::*;
use serde::Deserialize;
use serde_json::{json, Value};

pub const CHIPS: f64 = 100.0;
pub const GB: f64 = 1024.0 * 1024.0 * 1024.0;
pub const RANKS: &[u8] = b"23456789TJQKA";

#[derive(Deserialize, Clone)]
pub struct StreetSizes {
    pub oop_bet: String,
    pub oop_raise: String,
    pub ip_bet: String,
    pub ip_raise: String,
}

#[derive(Deserialize)]
pub struct SolveRequest {
    pub oop_range: String,
    pub ip_range: String,
    pub board: String,
    pub pot: f64,
    pub stack: f64,
    pub flop: StreetSizes,
    pub turn: StreetSizes,
    pub river: StreetSizes,
    #[serde(default)]
    pub turn_donk: String,
    #[serde(default)]
    pub river_donk: String,
    #[serde(default)]
    pub rake_pct: f64,
    #[serde(default)]
    pub rake_cap: f64,
    pub add_allin_threshold: f64,
    pub force_allin_threshold: f64,
    pub merging_threshold: f64,
    pub precision: String, // auto | full | compressed
    pub max_iterations: u32,
    pub target_pct: f32,
}

fn sizes(s: &StreetSizes) -> Result<[BetSizeOptions; 2], String> {
    let parse = |b: &str, r: &str, who| BetSizeOptions::try_from((b, r))
        .map_err(|e| format!("Tamanhos de aposta {who} inválidos ({b} / {r}): {e}"));
    Ok([parse(&s.oop_bet, &s.oop_raise, "OOP")?, parse(&s.ip_bet, &s.ip_raise, "IP")?])
}

fn donk(s: &str) -> Result<Option<DonkSizeOptions>, String> {
    if s.trim().is_empty() { return Ok(None); }
    DonkSizeOptions::try_from(s).map(Some).map_err(|e| format!("Donk inválido ({s}): {e}"))
}

pub fn build_game(r: &SolveRequest) -> Result<PostFlopGame, String> {
    let board: Vec<Card> = r.board.as_bytes().chunks(2)
        .map(|c| card_from_str(std::str::from_utf8(c).unwrap_or("")))
        .collect::<Result<_, _>>()?;
    if !(3..=5).contains(&board.len()) {
        return Err("O board precisa ter 3, 4 ou 5 cartas (ex.: Ks7d2c).".into());
    }
    let card_config = CardConfig {
        range: [
            r.oop_range.parse().map_err(|e| format!("Range OOP inválido: {e}"))?,
            r.ip_range.parse().map_err(|e| format!("Range IP inválido: {e}"))?,
        ],
        flop: [board[0], board[1], board[2]],
        turn: *board.get(3).unwrap_or(&NOT_DEALT),
        river: *board.get(4).unwrap_or(&NOT_DEALT),
    };
    let tree_config = TreeConfig {
        initial_state: [BoardState::Flop, BoardState::Turn, BoardState::River][board.len() - 3],
        starting_pot: (r.pot * CHIPS).round() as i32,
        effective_stack: (r.stack * CHIPS).round() as i32,
        rake_rate: r.rake_pct / 100.0,
        rake_cap: r.rake_cap * CHIPS,
        flop_bet_sizes: sizes(&r.flop)?,
        turn_bet_sizes: sizes(&r.turn)?,
        river_bet_sizes: sizes(&r.river)?,
        turn_donk_sizes: donk(&r.turn_donk)?,
        river_donk_sizes: donk(&r.river_donk)?,
        add_allin_threshold: r.add_allin_threshold,
        force_allin_threshold: r.force_allin_threshold,
        merging_threshold: r.merging_threshold,
    };
    PostFlopGame::with_config(card_config, ActionTree::new(tree_config)?)
}

/// Picks full 32-bit precision when it fits, else 16-bit compression. Returns (compress, GB).
pub fn choose_precision(raw_gb: f64, compressed_gb: f64, mode: &str, limit_gb: f64) -> Result<(bool, f64), String> {
    let (compress, need) = match mode {
        "full" => (false, raw_gb),
        "compressed" => (true, compressed_gb),
        _ if raw_gb <= limit_gb => (false, raw_gb),
        _ => (true, compressed_gb),
    };
    if need > limit_gb {
        return Err(format!(
            "A árvore precisa de {need:.2} GB (32-bit: {raw_gb:.2} GB, comprimida: {compressed_gb:.2} GB), \
             mas há {limit_gb:.2} GB disponíveis. Use menos bet sizes, estreite os ranges \
             ou escolha a precisão de 16 bits."));
    }
    Ok((compress, need))
}

pub fn card_str(c: Card) -> String {
    card_to_string(c).unwrap_or_default()
}

fn action_json(a: &Action) -> Value {
    let bb = |x: &i32| (*x as f64 / CHIPS * 100.0).round() / 100.0;
    let (kind, amount) = match a {
        Action::Fold => ("Fold", None),
        Action::Check => ("Check", None),
        Action::Call => ("Call", None),
        Action::Bet(x) => ("Bet", Some(bb(x))),
        Action::Raise(x) => ("Raise", Some(bb(x))),
        Action::AllIn(x) => ("Allin", Some(bb(x))),
        _ => ("?", None),
    };
    json!({ "kind": kind, "amount": amount })
}

/// 13x13 grid position: pairs on the diagonal, suited above it, offsuit below.
pub fn grid_cell(c1: Card, c2: Card) -> (usize, usize, String) {
    let (r1, r2) = ((c1 / 4) as usize, (c2 / 4) as usize);
    let (hi, lo) = (r1.max(r2), r1.min(r2));
    let (h, l) = (RANKS[hi] as char, RANKS[lo] as char);
    if hi == lo {
        (12 - hi, 12 - hi, format!("{h}{l}"))
    } else if c1 % 4 == c2 % 4 {
        (12 - hi, 12 - lo, format!("{h}{l}s"))
    } else {
        (12 - lo, 12 - hi, format!("{h}{l}o"))
    }
}

/// Label of a grid position, for hand classes absent from the range.
pub fn grid_label(row: usize, col: usize) -> String {
    let (a, b) = (RANKS[12 - row.min(col)] as char, RANKS[12 - row.max(col)] as char);
    match row.cmp(&col) {
        std::cmp::Ordering::Equal => format!("{a}{a}"),
        std::cmp::Ordering::Less => format!("{a}{b}s"),
        std::cmp::Ordering::Greater => format!("{a}{b}o"),
    }
}

fn player_name(p: usize) -> &'static str {
    if p == 0 { "OOP" } else { "IP" }
}

/// Weighted aggregate of combos: frequencies, per-action EV, EV and equity.
struct Agg {
    w: f64,      // averaging weight (normalized: reach x compatible opponent combos)
    combos: f64, // combos still held (reach), for display
    freqs: Vec<f64>,
    evs: Vec<f64>,
    ev: f64,
    equity: f64,
}

impl Agg {
    fn new(n_act: usize) -> Self {
        Agg { w: 0.0, combos: 0.0, freqs: vec![0.0; n_act], evs: vec![0.0; n_act], ev: 0.0, equity: 0.0 }
    }
    fn add(&mut self, w: f64, combos: f64, freqs: &[f64], evs: &[f64], ev: f64, equity: f64) {
        self.w += w;
        self.combos += combos;
        self.ev += w * ev;
        self.equity += w * equity;
        for a in 0..freqs.len() {
            self.freqs[a] += w * freqs[a];
            self.evs[a] += w * evs[a];
        }
    }
    fn json(&self) -> Value {
        let d = if self.w > 0.0 { self.w } else { 1.0 };
        json!({
            "weight": self.combos,
            "freqs": self.freqs.iter().map(|x| x / d).collect::<Vec<_>>(),
            "evs": self.evs.iter().map(|x| x / d).collect::<Vec<_>>(),
            "ev": self.ev / d, "equity": self.equity / d,
        })
    }
}

fn category_kind(name: &str) -> &'static str {
    if hands::MADE.contains(&name) { "made" }
    else if hands::DRAWS.contains(&name) { "draw" }
    else if hands::EQ_SIMPLE.iter().any(|(n, _)| *n == name) { "eq" }
    else { "eq_advanced" }
}

/// Range of one player at the current node: combos, 13x13 grid and average equity.
fn range_json(g: &PostFlopGame, p: usize) -> Value {
    let (hands, reach, w, eq) = (g.private_cards(p), g.weights(p), g.normalized_weights(p), g.equity(p));
    let mut grid = vec![(0.0f64, 0.0f64, 0.0f64); 169]; // (combos, weight, weight * equity)
    let mut combos = vec![];
    let (mut total_w, mut total_eq) = (0.0f64, 0.0f64);
    for k in 0..hands.len() {
        let wk = w[k] as f64;
        if wk <= 0.0 { continue; }
        let (c1, c2) = hands[k];
        let (row, col, _) = grid_cell(c1, c2);
        let cell = &mut grid[row * 13 + col];
        cell.0 += reach[k] as f64;
        cell.1 += wk;
        cell.2 += wk * eq[k] as f64;
        total_w += wk;
        total_eq += wk * eq[k] as f64;
        combos.push(json!({ "combo": format!("{}{}", card_str(c2), card_str(c1)), "weight": reach[k], "equity": eq[k] }));
    }
    json!({
        "player": player_name(p),
        "equity": if total_w > 0.0 { total_eq / total_w } else { 0.0 },
        "combos": combos,
        "grid": grid.iter().enumerate().map(|(i, &(c, w, e))| json!({
            "hand": grid_label(i / 13, i % 13), "weight": c, "equity": if w > 0.0 { e / w } else { 0.0 },
        })).collect::<Vec<_>>(),
    })
}

/// Validates a request, builds the tree and picks the precision that fits `limit_gb`.
/// Returns the tree (memory not yet allocated), whether to compress, and the GB it will use.
pub fn prepare(req: &SolveRequest, limit_gb: f64) -> Result<(PostFlopGame, bool, f64), String> {
    if req.max_iterations == 0 || req.pot <= 0.0 || req.stack <= 0.0 {
        return Err("Pote, stack e iterações precisam ser maiores que zero.".into());
    }
    let game = build_game(req)?;
    let (raw, comp) = game.memory_usage();
    let (compress, memory_gb) = choose_precision(raw as f64 / GB, comp as f64 / GB, &req.precision, limit_gb)?;
    Ok((game, compress, memory_gb))
}

/// JSON for the node reached by `history` (action indices and dealt card ids) from the root.
/// Errors on a history that does not fit this tree.
pub fn node_json(g: &mut PostFlopGame, history: &[usize]) -> Result<Value, String> {
    // Replay step by step so a bad history returns an error instead of panicking,
    // recording a label for every step (breadcrumbs, shareable links, trainer).
    g.back_to_root();
    let mut path = vec![];
    for &h in history {
        let step = if g.is_terminal_node() {
            None
        } else if g.is_chance_node() {
            if h < 52 && g.possible_cards() & (1u64 << h) != 0 && !can_deal(g) {
                // The saved tree ends here: the caller must solve this street as a subgame.
                let path_len = path.len();
                return Ok(json!({ "type": "resolve_needed", "prefix": &history[..path_len], "card": h }));
            }
            (h < 52 && g.possible_cards() & (1u64 << h) != 0).then(|| json!({ "card": card_str(h as Card) }))
        } else {
            g.available_actions().get(h)
                .map(|a| json!({ "player": player_name(g.current_player()), "action": action_json(a) }))
        };
        let Some(step) = step else {
            return Err("Histórico inválido para esta árvore.".into());
        };
        path.push(step);
        g.play(h);
    }

    let tb = g.total_bet_amount();
    let pot = (g.tree_config().starting_pot + tb[0] + tb[1]) as f64 / CHIPS;
    let board_cards = g.current_board();
    let board: Vec<String> = board_cards.iter().map(|&c| card_str(c)).collect();
    let base = json!({ "pot": pot, "board": board, "history": history, "path": path });

    if g.is_terminal_node() {
        return Ok(merge(base, json!({ "type": "terminal" })));
    }
    if g.is_chance_node() {
        let mask = g.possible_cards();
        let cards: Vec<Value> = (0..52u8)
            .map(|c| json!({ "id": c, "card": card_str(c), "possible": mask & (1u64 << c) != 0 }))
            .collect();
        return Ok(merge(base, json!({ "type": "chance", "cards": cards })));
    }

    g.cache_normalized_weights();
    let g: &PostFlopGame = g;
    let player = g.current_player();
    let actions: Vec<Value> = g.available_actions().iter().map(action_json).collect();
    let n_act = actions.len();
    let hands = g.private_cards(player);
    let n = hands.len();
    let strategy = g.strategy();
    let ev_detail = g.expected_values_detail(player);
    let ev = g.expected_values(player);
    let eq = g.equity(player);
    let weights = g.normalized_weights(player);
    let reach = g.weights(player);

    let mut cells: Vec<(Agg, Vec<Value>)> = (0..169).map(|_| (Agg::new(n_act), vec![])).collect();
    let names: Vec<&str> = hands::MADE.iter().chain(hands::DRAWS.iter()).copied()
        .chain(hands::EQ_SIMPLE.iter().chain(hands::EQ_ADVANCED.iter()).map(|(name, _)| *name))
        .collect();
    let mut categories: Vec<Agg> = names.iter().map(|_| Agg::new(n_act)).collect();
    let mut total = Agg::new(n_act);
    for k in 0..n {
        let w = weights[k] as f64;
        if w <= 0.0 { continue; }
        let (c1, c2) = hands[k];
        let (row, col, _) = grid_cell(c1, c2);
        let freqs: Vec<f64> = (0..n_act).map(|a| strategy[a * n + k] as f64).collect();
        let evs: Vec<f64> = (0..n_act).map(|a| ev_detail[a * n + k] as f64 / CHIPS).collect();
        let (combos, ev_k, eq_k) = (reach[k] as f64, ev[k] as f64 / CHIPS, eq[k] as f64);
        let made = hands::made_hand([c1, c2], &board_cards);
        let draws = hands::draws([c1, c2], &board_cards, made);
        let buckets = [hands::eq_bucket(&hands::EQ_SIMPLE, eq_k), hands::eq_bucket(&hands::EQ_ADVANCED, eq_k)];

        total.add(w, combos, &freqs, &evs, ev_k, eq_k);
        for name in std::iter::once(made).chain(draws.iter().copied()).chain(buckets) {
            let i = names.iter().position(|&x| x == name).unwrap();
            categories[i].add(w, combos, &freqs, &evs, ev_k, eq_k);
        }
        let cell = &mut cells[row * 13 + col];
        cell.0.add(w, combos, &freqs, &evs, ev_k, eq_k);
        cell.1.push(json!({
            "combo": format!("{}{}", card_str(c2), card_str(c1)), "weight": combos,
            "freqs": freqs, "evs": evs, "ev": ev_k, "equity": eq_k, "made": made, "draws": draws, "buckets": buckets,
        }));
    }
    let grid: Vec<Value> = cells.into_iter().enumerate().map(|(i, (agg, combos))| {
        merge(agg.json(), json!({ "hand": grid_label(i / 13, i % 13), "combos": combos }))
    }).collect();
    let categories: Vec<Value> = names.iter().zip(&categories)
        .filter(|(_, agg)| agg.w > 0.0)
        .map(|(name, agg)| merge(agg.json(), json!({
            "name": name, "kind": category_kind(name),
        })))
        .collect();
    let summary = total.json();
    Ok(merge(base, json!({
        "type": "action",
        "player": player_name(player),
        "actions": actions,
        "summary": summary["freqs"],
        "summary_evs": summary["evs"],
        "total_combos": total.combos,
        "grid": grid,
        "categories": categories,
        "ranges": [range_json(g, 0), range_json(g, 1)],
    })))
}

/// Index of a two-card combo in a 1326-entry range, the same layout `Range::raw_data` uses.
pub fn combo_index(c1: Card, c2: Card) -> usize {
    let (lo, hi) = (c1.min(c2) as usize, c1.max(c2) as usize);
    lo * (101 - lo) / 2 + hi - 1
}

/// Serializes a solved flop tree keeping only the flop decisions (small enough to ship
/// in a static site). Turn and river are re-solved on demand with [`subgame_at_chance`].
pub fn save_flop(g: &mut PostFlopGame, memo: &str) -> Result<Vec<u8>, String> {
    g.set_target_storage_mode(BoardState::Flop)?;
    let mut bytes = Vec::new();
    save_data_into_std_write(g, memo, &mut bytes, None)?;
    Ok(bytes)
}

/// Loads a tree saved by [`save_flop`] (or a full save). Returns the tree and its memo.
pub fn load(bytes: &[u8]) -> Result<(PostFlopGame, String), String> {
    load_data_from_std_read(&mut &bytes[..], None)
}

/// Whether the tree can deal the next card at this chance node, or the stored data ends here.
fn can_deal(g: &PostFlopGame) -> bool {
    let dealing_turn = g.current_board().len() == 3;
    match g.storage_mode() {
        BoardState::Flop => false,
        BoardState::Turn => dealing_turn,
        BoardState::River => true,
    }
}

/// Builds the turn (or river) subgame that starts after `prefix` (which must end at a chance
/// node) with `card` dealt: both players' ranges are their reach at that node, the pot and
/// stacks are carried over, and the bet sizes are the parent tree's. Memory is not allocated.
pub fn subgame_at_chance(g: &mut PostFlopGame, prefix: &[usize], card: usize) -> Result<PostFlopGame, String> {
    g.back_to_root();
    for &h in prefix {
        if g.is_terminal_node() { return Err("Histórico inválido para esta árvore.".into()); }
        g.play(h);
    }
    if !g.is_chance_node() || card >= 52 || g.possible_cards() & (1u64 << card) == 0 {
        return Err("Carta inválida para este ponto da árvore.".into());
    }
    let ranges: Vec<Range> = (0..2).map(|p| {
        let mut data = vec![0.0f32; 1326];
        for (&(c1, c2), &w) in g.private_cards(p).iter().zip(g.weights(p)) {
            data[combo_index(c1, c2)] = w.clamp(0.0, 1.0);
        }
        Range::from_raw_data(&data)
    }).collect::<Result<_, _>>()?;

    let mut board = g.current_board();
    board.push(card as Card);
    let tb = g.total_bet_amount();
    let parent = g.tree_config().clone();
    let card_config = CardConfig {
        range: [ranges[0].clone(), ranges[1].clone()],
        flop: [board[0], board[1], board[2]],
        turn: board[3],
        river: *board.get(4).unwrap_or(&NOT_DEALT),
    };
    let tree_config = TreeConfig {
        initial_state: if board.len() == 4 { BoardState::Turn } else { BoardState::River },
        starting_pot: parent.starting_pot + tb[0] + tb[1],
        effective_stack: parent.effective_stack - tb[0].max(tb[1]),
        ..parent
    };
    PostFlopGame::with_config(card_config, ActionTree::new(tree_config)?)
}

pub fn merge(mut a: Value, b: Value) -> Value {
    if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn street(bet: &str) -> StreetSizes {
        StreetSizes { oop_bet: bet.into(), oop_raise: "".into(), ip_bet: bet.into(), ip_raise: "".into() }
    }

    fn river_request(oop: &str, ip: &str, max_iterations: u32) -> SolveRequest {
        SolveRequest {
            oop_range: oop.into(), ip_range: ip.into(), board: "Ks7d2c8h3s".into(),
            pot: 10.0, stack: 50.0, flop: street("50%"), turn: street("50%"), river: street("50%"),
            turn_donk: "".into(), river_donk: "".into(), rake_pct: 5.0, rake_cap: 3.0,
            add_allin_threshold: 1.5, force_allin_threshold: 0.15, merging_threshold: 0.1,
            precision: "auto".into(), max_iterations, target_pct: 0.5,
        }
    }

    fn solved(req: &SolveRequest) -> PostFlopGame {
        let (mut g, compress, _) = prepare(req, 4.0).unwrap();
        g.allocate_memory(compress);
        solve(&mut g, req.max_iterations, 0.05, false);
        g
    }

    #[test]
    fn grid_layout() {
        let c = |s| card_from_str(s).unwrap();
        assert_eq!(grid_cell(c("As"), c("Ah")), (0, 0, "AA".into()));
        assert_eq!(grid_cell(c("Ks"), c("As")), (0, 1, "AKs".into()));
        assert_eq!(grid_cell(c("Kd"), c("As")), (1, 0, "AKo".into()));
        assert_eq!(grid_cell(c("2c"), c("3d")), (12, 11, "32o".into()));
        for (row, col, label) in [(0, 0, "AA"), (0, 1, "AKs"), (1, 0, "AKo"), (12, 11, "32o")] {
            assert_eq!(grid_label(row, col), label);
        }
    }

    #[test]
    fn precision_choice() {
        assert_eq!(choose_precision(2.0, 1.0, "auto", 4.0), Ok((false, 2.0)));
        assert_eq!(choose_precision(6.0, 3.0, "auto", 4.0), Ok((true, 3.0)));
        assert!(choose_precision(6.0, 3.0, "full", 4.0).is_err());
        assert!(choose_precision(10.0, 5.0, "auto", 4.0).is_err());
    }

    #[test]
    fn prepare_rejects_bad_input() {
        assert!(prepare(&river_request("AA", "KK", 0), 4.0).is_err());
        assert!(prepare(&river_request("AA-", "KK", 10), 4.0).is_err());
    }

    #[test]
    fn river_solve_and_node() {
        let mut g = solved(&river_request("AA,KK,QQ,AKs", "JJ,TT,AQs", 100));
        let v = node_json(&mut g, &[]).unwrap();
        assert_eq!(v["type"], "action");
        assert_eq!(v["grid"].as_array().unwrap().len(), 169);
        let s: f64 = v["summary"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).sum();
        assert!((s - 1.0).abs() < 1e-3, "frequencies sum to {s}");
        assert!(node_json(&mut g, &[99]).is_err());

        // Categories cover every combo exactly once by made hand.
        let made: f64 = v["categories"].as_array().unwrap().iter()
            .filter(|c| c["kind"] == "made").map(|c| c["weight"].as_f64().unwrap()).sum();
        assert!((made - v["total_combos"].as_f64().unwrap()).abs() < 1e-6);
        // Each equity bucket family also partitions the range.
        for kind in ["eq", "eq_advanced"] {
            let sum: f64 = v["categories"].as_array().unwrap().iter()
                .filter(|c| c["kind"] == kind).map(|c| c["weight"].as_f64().unwrap()).sum();
            assert!((sum - v["total_combos"].as_f64().unwrap()).abs() < 1e-6, "{kind}: {sum}");
        }
        assert_eq!(v["ranges"].as_array().unwrap().len(), 2);
        assert_eq!(v["summary_evs"].as_array().unwrap().len(), v["actions"].as_array().unwrap().len());

        let child = node_json(&mut g, &[0]).unwrap();
        assert_eq!(child["path"][0]["player"], "OOP");
        assert_eq!(child["path"][0]["action"], v["actions"][0]);
    }

    #[test]
    fn combo_counts_are_real_combos() {
        let mut g = solved(&river_request("AA,KK", "QQ", 10));
        let v = node_json(&mut g, &[]).unwrap();
        // AA has 6 combos, KK has 3 (Ks is on the board).
        assert!((v["total_combos"].as_f64().unwrap() - 9.0).abs() < 1e-6, "{}", v["total_combos"]);
        assert!((v["grid"][14]["weight"].as_f64().unwrap() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn combo_index_matches_solver_layout() {
        let c = |s| card_from_str(s).unwrap();
        let r: Range = "AsAh".parse().unwrap();
        assert_eq!(r.raw_data()[combo_index(c("As"), c("Ah"))], 1.0);
        assert_eq!(r.raw_data().iter().sum::<f32>(), 1.0);
        assert_eq!(combo_index(c("2c"), c("2d")), 0);
        assert_eq!(combo_index(c("As"), c("Ah")), combo_index(c("Ah"), c("As")));
    }

    #[test]
    fn flop_library_roundtrip_and_turn_subgame() {
        let mut req = river_request("AA,KK,AKs", "QQ,JJ,AQs", 30);
        req.board = "Ks7d2c".into();
        let mut g = solved(&req);
        let root = node_json(&mut g, &[]).unwrap();

        let bytes = save_flop(&mut g, "teste").unwrap();
        let (mut lib, memo) = load(&bytes).unwrap();
        assert_eq!(memo, "teste");
        assert_eq!(node_json(&mut lib, &[]).unwrap()["summary"], root["summary"]);

        // Check-check reaches the turn deal; dealing needs a subgame.
        let chance = node_json(&mut lib, &[0, 0]).unwrap();
        assert_eq!(chance["type"], "chance");
        let card = chance["cards"].as_array().unwrap().iter().find(|c| c["possible"] == true).unwrap()["id"].as_u64().unwrap() as usize;
        let need = node_json(&mut lib, &[0, 0, card]).unwrap();
        assert_eq!(need["type"], "resolve_needed");
        assert_eq!(need["prefix"], json!([0, 0]));

        let mut sub = subgame_at_chance(&mut lib, &[0, 0], card).unwrap();
        sub.allocate_memory(false);
        solve(&mut sub, 30, 0.05, false);
        let turn = node_json(&mut sub, &[]).unwrap();
        assert_eq!(turn["type"], "action");
        assert_eq!(turn["board"].as_array().unwrap().len(), 4);
        assert_eq!(turn["pot"], chance["pot"]);
    }

    /// Every preflop preset shipped in the UI must be a range the solver accepts.
    #[test]
    fn ui_presets_parse() {
        let html = include_str!("../static/index.html");
        let quoted: Vec<&str> = html.match_indices("oop: '").chain(html.match_indices("ip: '"))
            .map(|(i, m)| { let s = &html[i + m.len()..]; &s[..s.find('\'').unwrap()] })
            .collect();
        assert_eq!(quoted.len(), 44, "22 presets x 2 ranges");
        for r in quoted {
            r.parse::<Range>().unwrap_or_else(|e| panic!("{r}: {e}"));
        }
    }
}
