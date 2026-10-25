//! Leaderboard model, kept in sync like the original front end: counts are replaced by a snapshot
//! (the server's `LeaderboardResponse`, then the fully loaded ownerships) and moved by every live
//! update (+1 for the new owner, -1 for the previous one).

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct LeaderboardModel {
    counts: HashMap<String, i64>,
}

impl LeaderboardModel {
    /// Replaces all counts.
    pub fn set_snapshot<I: IntoIterator<Item = (String, u32)>>(&mut self, entries: I) {
        self.counts = entries
            .into_iter()
            .map(|(code, count)| (code, count as i64))
            .collect();
    }

    /// A tile moved from `previous` (if any) to `new`.
    pub fn register_click(&mut self, previous: Option<&str>, new: &str) {
        if previous == Some(new) {
            return;
        }
        *self.counts.entry(new.to_string()).or_insert(0) += 1;
        if let Some(previous) = previous {
            if let Some(count) = self.counts.get_mut(previous) {
                *count -= 1;
            }
        }
    }

    /// Countries with at least one tile, by descending tile count (ties by code).
    pub fn entries(&self) -> Vec<(String, u32)> {
        let mut entries: Vec<(String, u32)> = self
            .counts
            .iter()
            .filter(|(_, &count)| count > 0)
            .map(|(code, &count)| (code.clone(), count as u32))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries
    }
}

/// Share of the map, formatted like the original (`toFixed(2)`).
pub fn percentage(tiles: u32, tile_count: usize) -> String {
    if tile_count == 0 {
        return "0.00".to_string();
    }
    format!("{:.2}", tiles as f64 / tile_count as f64 * 100.0)
}

/// Truncates a country name to 18 characters like the original leaderboard.
pub fn short_name(name: &str) -> String {
    const MAX: usize = 18;
    let truncated: String = name.chars().take(MAX).collect();
    if truncated.chars().count() == MAX {
        truncated.trim().to_string()
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_move_counts() {
        let mut lb = LeaderboardModel::default();
        lb.set_snapshot(vec![("fr".to_string(), 10), ("de".to_string(), 5)]);
        lb.register_click(Some("fr"), "de");
        lb.register_click(None, "us");
        lb.register_click(Some("us"), "us");
        assert_eq!(
            lb.entries(),
            vec![("fr".to_string(), 9), ("de".to_string(), 6), ("us".to_string(), 1)]
        );
    }

    #[test]
    fn empty_countries_are_hidden() {
        let mut lb = LeaderboardModel::default();
        lb.set_snapshot(vec![("fr".to_string(), 1)]);
        lb.register_click(Some("fr"), "de");
        assert_eq!(lb.entries(), vec![("de".to_string(), 1)]);
    }

    #[test]
    fn formatting() {
        assert_eq!(percentage(1, 3), "33.33");
        assert_eq!(percentage(0, 0), "0.00");
        assert_eq!(short_name("short"), "short");
        assert_eq!(short_name("🇨🇫 Central African Republic"), "🇨🇫 Central African");
    }
}
