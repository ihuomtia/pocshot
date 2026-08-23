pub const SNAP_DISTANCE: f32 = 8.0;

pub fn snap_pos(pos: f32, lines: &[f32], threshold: f32) -> f32 {
    lines
        .iter()
        .map(|&l| (l, (l - pos).abs()))
        .filter(|&(_, d)| d <= threshold)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(l, _)| l)
        .unwrap_or(pos)
}
