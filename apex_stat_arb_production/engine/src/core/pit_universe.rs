use std::collections::HashMap;

/// Point-In-Time Universe Filter
/// Strictly ranks tokens by historical dollar volume (Price * Volume) over the lookback window.
pub struct PitUniverseFilter {
    pub top_n: usize,
}

impl PitUniverseFilter {
    pub fn new(top_n: usize) -> Self {
        Self { top_n }
    }

    pub fn select_top_liquid_universe(&self, volume_map: &HashMap<String, f64>) -> Vec<String> {
        let mut sorted: Vec<(String, f64)> = volume_map.iter().map(|(k, v)| (k.clone(), *v)).collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(self.top_n).map(|(s, _)| s).collect()
    }
}
