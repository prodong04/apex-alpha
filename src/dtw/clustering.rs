use super::distance::{dtw_distance, zscore_normalize};

#[derive(Debug, Clone)]
pub struct ClusterBasket {
    pub cluster_id: usize,
    pub medoid_symbol: String,
    pub symbols: Vec<String>,
    pub avg_intra_distance: f64,
}

/// Computes an N x N DTW Distance Matrix across multiple asset normalized price series.
pub fn compute_dtw_distance_matrix(
    symbols: &[String],
    series_map: &[Vec<f64>], // Same length as symbols, each having same length T
    window_radius: usize,
) -> Vec<Vec<f64>> {
    let n = symbols.len();
    let mut normalized: Vec<Vec<f64>> = Vec::with_capacity(n);
    for s in series_map {
        normalized.push(zscore_normalize(s));
    }

    let mut matrix = vec![vec![0.0; n]; n];

    for i in 0..n {
        for j in (i + 1)..n {
            let dist = dtw_distance(&normalized[i], &normalized[j], window_radius);
            matrix[i][j] = dist;
            matrix[j][i] = dist;
        }
    }

    matrix
}

/// Agglomerative Hierarchical Clustering (Complete-Linkage) to form K dynamic baskets.
pub fn cluster_into_baskets(
    symbols: &[String],
    dist_matrix: &[Vec<f64>],
    num_clusters: usize,
) -> Vec<ClusterBasket> {
    let n = symbols.len();
    if n == 0 {
        return Vec::new();
    }
    let k = num_clusters.min(n).max(1);

    // Initial state: Each item is its own cluster
    let mut clusters: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();

    while clusters.len() > k {
        let mut min_dist = f64::INFINITY;
        let mut best_pair = (0, 1);

        for i in 0..clusters.len() {
            for j in (i + 1)..clusters.len() {
                // Complete-linkage: maximum distance between points in cluster i and cluster j
                let mut max_d = 0.0f64;
                for &u in &clusters[i] {
                    for &v in &clusters[j] {
                        if dist_matrix[u][v] > max_d {
                            max_d = dist_matrix[u][v];
                        }
                    }
                }

                if max_d < min_dist {
                    min_dist = max_d;
                    best_pair = (i, j);
                }
            }
        }

        // Merge clusters[best_pair.1] into clusters[best_pair.0]
        let (c1, c2) = best_pair;
        let mut to_merge = clusters.remove(c2);
        clusters[c1].append(&mut to_merge);
    }

    // Build ClusterBasket objects & find Medoid
    let mut result = Vec::new();
    for (cid, member_indices) in clusters.iter().enumerate() {
        let mut member_symbols = Vec::new();
        for &idx in member_indices {
            member_symbols.push(symbols[idx].clone());
        }

        // Find medoid (member with minimum total distance to other members)
        let mut best_medoid = member_symbols[0].clone();
        let mut min_total_d = f64::INFINITY;

        for &u in member_indices {
            let mut total_d = 0.0;
            for &v in member_indices {
                total_d += dist_matrix[u][v];
            }
            if total_d < min_total_d {
                min_total_d = total_d;
                best_medoid = symbols[u].clone();
            }
        }

        let avg_intra = if member_indices.len() > 1 {
            min_total_d / (member_indices.len() - 1) as f64
        } else {
            0.0
        };

        result.push(ClusterBasket {
            cluster_id: cid,
            medoid_symbol: best_medoid,
            symbols: member_symbols,
            avg_intra_distance: avg_intra,
        });
    }

    result
}
