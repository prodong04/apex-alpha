/// Z-Score normalize a time series so price levels don't bias shape comparison.
pub fn zscore_normalize(series: &[f64]) -> Vec<f64> {
    let n = series.len() as f64;
    if n <= 1.0 {
        return vec![0.0; series.len()];
    }

    let mean = series.iter().sum::<f64>() / n;
    let var = series.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt();

    if std < 1e-8 {
        vec![0.0; series.len()]
    } else {
        series.iter().map(|x| (x - mean) / std).collect()
    }
}

/// Computes Dynamic Time Warping (DTW) distance with Sakoe-Chiba constraint window.
/// `window_radius`: max time warping lag allowed (e.g., 5-15 bars).
pub fn dtw_distance(s1: &[f64], s2: &[f64], window_radius: usize) -> f64 {
    let n = s1.len();
    let m = s2.len();
    if n == 0 || m == 0 {
        return f64::MAX;
    }

    let w = window_radius.max((n as isize - m as isize).unsigned_abs());
    let mut prev_row = vec![f64::INFINITY; m + 1];
    let mut curr_row = vec![f64::INFINITY; m + 1];
    prev_row[0] = 0.0;

    for i in 1..=n {
        curr_row.fill(f64::INFINITY);
        let j_start = 1.max(if i > w { i - w } else { 1 });
        let j_end = m.min(i + w);

        for j in j_start..=j_end {
            let cost = (s1[i - 1] - s2[j - 1]).abs();
            let min_prev = prev_row[j]
                .min(curr_row[j - 1])
                .min(prev_row[j - 1]);
            curr_row[j] = cost + min_prev;
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_series() {
        let s = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let z = zscore_normalize(&s);
        let dist = dtw_distance(&z, &z, 5);
        assert!(dist < 1e-6);
    }
}
