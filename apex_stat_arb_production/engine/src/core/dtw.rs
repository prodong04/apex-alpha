/// Sakoe-Chiba Constrained Dynamic Time Warping (DTW) with O(1) Memory Allocation
/// High-performance L1-cache friendly 2-row rolling DP buffer.

pub fn zscore_normalize(series: &[f64]) -> Vec<f64> {
    let n = series.len();
    if n == 0 {
        return Vec::new();
    }
    let mean: f64 = series.iter().sum::<f64>() / n as f64;
    let var: f64 = series.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n as f64;
    let std = var.sqrt();
    if std < 1e-8 {
        vec![0.0; n]
    } else {
        series.iter().map(|&x| (x - mean) / std).collect()
    }
}

pub fn dtw_distance(s1: &[f64], s2: &[f64], window: usize) -> f64 {
    let n = s1.len();
    let m = s2.len();
    if n == 0 || m == 0 {
        return f64::INFINITY;
    }

    let mut prev_row = vec![f64::INFINITY; m + 1];
    let mut curr_row = vec![f64::INFINITY; m + 1];

    prev_row[0] = 0.0;

    for i in 1..=n {
        curr_row.fill(f64::INFINITY);
        let j_start = if i > window { i - window } else { 1 };
        let j_end = (i + window).min(m);

        for j in j_start..=j_end {
            let cost = (s1[i - 1] - s2[j - 1]).abs();
            let min_prev = prev_row[j].min(curr_row[j - 1]).min(prev_row[j - 1]);
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
    fn test_dtw_identity() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let d = dtw_distance(&a, &a, 2);
        assert!(d < 1e-6);
    }

    #[test]
    fn test_zscore() {
        let a = vec![10.0, 10.0, 10.0];
        let z = zscore_normalize(&a);
        assert_eq!(z, vec![0.0, 0.0, 0.0]);
    }
}
