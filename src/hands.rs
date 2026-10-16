//! Hand categories relative to the board: the best made hand and the draws.
//! Card ids follow postflop-solver: `4 * rank + suit`, rank 0 = deuce .. 12 = ace.

use postflop_solver::Card;

pub const MADE: [&str; 14] = [
    "Straight flush", "Quads", "Full house", "Flush", "Straight", "Set", "Trips", "Two pair",
    "Overpair", "Top pair", "Middle pair", "Weak pair", "Ace high", "No made hand",
];
pub const DRAWS: [&str; 6] = [
    "Flush draw", "OESD", "Gutshot", "Backdoor flush draw", "Backdoor straight draw", "Overcards",
];

/// Equity buckets (simple and advanced), as (name, lower bound inclusive).
pub const EQ_SIMPLE: [(&str, f64); 4] = [
    ("Equity 75–100%", 0.75), ("Equity 50–75%", 0.50), ("Equity 33–50%", 0.33), ("Equity 0–33%", 0.0),
];
pub const EQ_ADVANCED: [(&str, f64); 7] = [
    ("Equity 90–100%", 0.90), ("Equity 80–90%", 0.80), ("Equity 70–80%", 0.70), ("Equity 60–70%", 0.60),
    ("Equity 50–60%", 0.50), ("Equity 25–50%", 0.25), ("Equity 0–25%", 0.0),
];

/// Name of the bucket an equity (0..1) falls into.
pub fn eq_bucket(buckets: &[(&'static str, f64)], equity: f64) -> &'static str {
    buckets.iter().find(|(_, lo)| equity >= *lo).map_or(buckets[buckets.len() - 1].0, |(name, _)| *name)
}

const ACE: usize = 12;

fn rank_counts(cards: &[Card]) -> [u8; 13] {
    let mut c = [0; 13];
    cards.iter().for_each(|&x| c[(x / 4) as usize] += 1);
    c
}

fn suit_counts(cards: &[Card]) -> [u8; 4] {
    let mut c = [0; 4];
    cards.iter().for_each(|&x| c[(x % 4) as usize] += 1);
    c
}

fn rank_mask(cards: &[Card]) -> u16 {
    cards.iter().fold(0, |m, &x| m | 1 << (x / 4))
}

/// Highest card of the best straight in a rank mask (the wheel counts as 5-high).
fn straight_high(mask: u16) -> Option<usize> {
    let window = |top: usize| (top - 4..=top).all(|r| mask & 1 << r != 0);
    (4..=12).rev().find(|&top| window(top))
        .or_else(|| (mask & 1 << ACE != 0 && (0..4).all(|r| mask & 1 << r != 0)).then_some(3))
}

fn full_house(counts: &[u8; 13]) -> bool {
    (0..13).any(|t| counts[t] >= 3 && (0..13).any(|p| p != t && counts[p] >= 2))
}

/// Best made hand that uses at least one hole card.
pub fn made_hand(hole: [Card; 2], board: &[Card]) -> &'static str {
    let all: Vec<Card> = hole.iter().chain(board).copied().collect();
    let (rc_all, rc_board) = (rank_counts(&all), rank_counts(board));
    let suits = suit_counts(&all);

    let flush_suit = (0..4u8).find(|&s| suits[s as usize] >= 5 && hole.iter().any(|&h| h % 4 == s));
    if let Some(s) = flush_suit {
        let suited: Vec<Card> = all.iter().copied().filter(|&c| c % 4 == s).collect();
        if straight_high(rank_mask(&suited)).is_some() {
            return "Straight flush";
        }
    }
    if (0..13).any(|r| rc_all[r] == 4 && rc_board[r] < 4) {
        return "Quads";
    }
    if full_house(&rc_all) && !full_house(&rc_board) {
        return "Full house";
    }
    if flush_suit.is_some() {
        return "Flush";
    }
    if straight_high(rank_mask(&all)) > straight_high(rank_mask(board)) {
        return "Straight";
    }

    let (r1, r2) = ((hole[0] / 4) as usize, (hole[1] / 4) as usize);
    let mut board_ranks: Vec<usize> = (0..13).rev().filter(|&r| rc_board[r] > 0).collect();
    board_ranks.dedup();
    let (top, bottom) = (board_ranks[0], *board_ranks.last().unwrap());

    if r1 == r2 {
        return if rc_board[r1] >= 1 { "Set" }
            else if r1 > top { "Overpair" }
            else if r1 > bottom { "Middle pair" }
            else { "Weak pair" };
    }
    if rc_board[r1] >= 2 || rc_board[r2] >= 2 {
        return "Trips";
    }
    match (rc_board[r1] > 0, rc_board[r2] > 0) {
        (true, true) => return "Two pair",
        (true, false) | (false, true) => {
            let hit = if rc_board[r1] > 0 { r1 } else { r2 };
            return match board_ranks.iter().position(|&r| r == hit) {
                Some(0) => "Top pair",
                Some(1) => "Middle pair",
                _ => "Weak pair",
            };
        }
        _ => {}
    }
    if r1 == ACE || r2 == ACE { "Ace high" } else { "No made hand" }
}

/// Whether two more cards (turn and river) can complete a straight that needs a hole card.
fn backdoor_straight(m_all: u16, m_board: u16) -> bool {
    (0..13).any(|a| (a + 1..13).any(|b| {
        let extra = 1u16 << a | 1 << b;
        m_all & extra != extra && straight_high(m_all | extra) > straight_high(m_board | extra)
    }))
}

/// Draws still live with cards to come (none on the river).
pub fn draws(hole: [Card; 2], board: &[Card], made: &str) -> Vec<&'static str> {
    let mut out = vec![];
    if board.len() >= 5 {
        return out;
    }
    let all: Vec<Card> = hole.iter().chain(board).copied().collect();
    let suits = suit_counts(&all);
    let uses_hole = |s: usize| hole.iter().any(|&h| (h % 4) as usize == s);
    let has_flush = matches!(made, "Flush" | "Straight flush");

    if !has_flush && (0..4).any(|s| suits[s] == 4 && uses_hole(s)) {
        out.push("Flush draw");
    } else if !has_flush && board.len() == 3 && (0..4).any(|s| suits[s] == 3 && uses_hole(s)) {
        out.push("Backdoor flush draw");
    }

    if !matches!(made, "Straight" | "Straight flush") {
        let (m_all, m_board) = (rank_mask(&all), rank_mask(board));
        // A rank is an out when it completes a straight that needs a hole card.
        let outs = (0..13)
            .filter(|&r| m_all & 1 << r == 0)
            .filter(|&r| straight_high(m_all | 1 << r) > straight_high(m_board | 1 << r))
            .count();
        match outs {
            0 if board.len() == 3 && backdoor_straight(m_all, m_board) => out.push("Backdoor straight draw"),
            0 => {}
            1 => out.push("Gutshot"),
            _ => out.push("OESD"),
        }
    }

    let board_top = board.iter().map(|&c| c / 4).max().unwrap_or(0);
    if matches!(made, "Ace high" | "No made hand") && hole.iter().all(|&h| h / 4 > board_top) {
        out.push("Overcards");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use postflop_solver::card_from_str;

    fn cat(hole: &str, board: &str) -> (&'static str, Vec<&'static str>) {
        let c = |s: &str| card_from_str(s).unwrap();
        let h = [c(&hole[0..2]), c(&hole[2..4])];
        let b: Vec<Card> = board.as_bytes().chunks(2).map(|x| c(std::str::from_utf8(x).unwrap())).collect();
        let made = made_hand(h, &b);
        (made, draws(h, &b, made))
    }

    #[test]
    fn equity_buckets() {
        assert_eq!(eq_bucket(&EQ_SIMPLE, 0.80), "Equity 75–100%");
        assert_eq!(eq_bucket(&EQ_SIMPLE, 0.50), "Equity 50–75%");
        assert_eq!(eq_bucket(&EQ_SIMPLE, 0.10), "Equity 0–33%");
        assert_eq!(eq_bucket(&EQ_ADVANCED, 0.95), "Equity 90–100%");
        assert_eq!(eq_bucket(&EQ_ADVANCED, 0.30), "Equity 25–50%");
    }

    #[test]
    fn made_hands() {
        assert_eq!(cat("AsAh", "Ks7d2c").0, "Overpair");
        assert_eq!(cat("7s7h", "Ks7d2c").0, "Set");
        assert_eq!(cat("KhQh", "Ks7d2c").0, "Top pair");
        assert_eq!(cat("8h7h", "Ks7d2c").0, "Middle pair");
        assert_eq!(cat("Ah2h", "Ks7d2c").0, "Weak pair");
        assert_eq!(cat("Kh7h", "Ks7d2c").0, "Two pair");
        assert_eq!(cat("AhQh", "Ks7d2c").0, "Ace high");
        assert_eq!(cat("4s3s", "Ks7d2c").0, "No made hand");
        assert_eq!(cat("6s5s", "8d7c4h").0, "Straight");
        assert_eq!(cat("Ah3h", "4h5d2c").0, "Straight"); // wheel
        assert_eq!(cat("AhTh", "Kh7h2h").0, "Flush");
        assert_eq!(cat("7h2h", "7s7d2c").0, "Full house");
        assert_eq!(cat("AsKd", "7s7d7c7h").0, "Ace high"); // quads on board belong to everyone
    }

    #[test]
    fn draw_detection() {
        assert_eq!(cat("AhTh", "Kh7h2c").1, vec!["Flush draw"]);
        assert_eq!(cat("9s8s", "Ts7c2h").1, vec!["Backdoor flush draw", "OESD"]);
        assert_eq!(cat("9c5d", "8h7s2d").1, vec!["Gutshot"]);
        assert_eq!(cat("AcQd", "9h7s2d").1, vec!["Backdoor straight draw", "Overcards"]);
        assert_eq!(cat("9c8d", "Kh7s2d").1, vec!["Backdoor straight draw"]);
        assert!(cat("KcKd", "8h4s2c").1.is_empty());
        assert!(cat("AhTh", "Kh7h2c3d4s").1.is_empty()); // river: no draws
    }
}
