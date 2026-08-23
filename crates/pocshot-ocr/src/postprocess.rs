//! Confidence filtering, line grouping and box merging.
//!
//! All functions are pure and operate on `TextRegion` values that already live
//! in *original screenshot coordinates* (the detector scales them back before
//! handing them to these stages).

use crate::{RectF32, TextRegion};

/// Drop regions whose confidence is below `min_confidence`. Regions with NaN
/// confidence are kept (`NaN` comparisons are false) so a caller that assigns
/// `0.0` explicitly is the only way to lift a box out of the default set;
/// detectors without per-box scores set a meaningful default before calling.
pub fn filter_by_confidence(regions: Vec<TextRegion>, min_confidence: f32) -> Vec<TextRegion> {
    regions
        .into_iter()
        .filter(|r| r.confidence >= min_confidence)
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct MergeConfig {
    /// Minimum vertical overlap (as a fraction of the smaller box height)
    /// for two boxes to be considered part of the same line.
    pub min_vertical_overlap: f32,
    /// Max difference between box center-Y values as a fraction of the
    /// smaller box height; boxes whose baselines drift more than this are not
    /// merged even if they overlap horizontally.
    pub max_center_y_drift: f32,
    /// Max horizontal gap between two boxes that still get merged, as a
    /// fraction of the smaller box height.
    pub max_gap_to_height: f32,
    /// Horizontal overlap required to merge even when separated by a gap:
    /// if two boxes overlap in X they merge at any gap (they are double boxes
    /// on the same text run).
    pub min_horizontal_overlap: f32,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            min_vertical_overlap: 0.6,
            max_center_y_drift: 0.6,
            max_gap_to_height: 6.0,
            min_horizontal_overlap: 0.1,
        }
    }
}

/// Merge boxes that belong to the same text line into a single region.
///
/// Boxes are first normalized to a canonical orientation (top < bottom,
/// left < right) and grouped greedily by line: a box joins the current line
/// group when its vertical overlap with the *last* box in the group is high
/// and its center-Y is close. Boxes that are horizontally overlapping but
/// vertically offset are never merged (they are on different lines).
///
/// Thresholds are relative to box height, so the same configuration works for
/// large titles and small captions alike.
pub fn merge_text_regions(regions: Vec<TextRegion>, cfg: MergeConfig) -> Vec<TextRegion> {
    let mut regs: Vec<TextRegion> = regions
        .into_iter()
        .filter(|r| r.rect.y1 >= r.rect.y0 && r.rect.x1 >= r.rect.x0)
        .collect();

    // Sort by top edge, then left edge, so a line group is visited in reading
    // order and merges accumulate left-to-right (stable for equal keys).
    regs.sort_by(|a, b| {
        a.rect
            .y0
            .partial_cmp(&b.rect.y0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    regs.sort_by(|a, b| {
        let by_y = a
            .rect
            .y0
            .partial_cmp(&b.rect.y0)
            .unwrap_or(std::cmp::Ordering::Equal);
        if by_y != std::cmp::Ordering::Equal {
            return by_y;
        }
        a.rect
            .x0
            .partial_cmp(&b.rect.x0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut merged: Vec<TextRegion> = Vec::new();
    let mut group: Vec<RectF32> = Vec::new();
    let mut group_conf: Vec<f32> = Vec::new();
    let mut group_text: Vec<Option<String>> = Vec::new();

    let same_line = |a: &RectF32, b: &RectF32| -> bool {
        let h = a.height().min(b.height()).max(1.0);
        let overlap = a.vertical_overlap(b);
        let center_gap = (a.center_y() - b.center_y()).abs();
        let x_gap = (b.x0 - a.x1).max(0.0);

        let overlapping_x = a.x0 < b.x1 && b.x0 < a.x1;
        let same_line_by_overlap =
            overlap >= cfg.min_vertical_overlap && center_gap <= cfg.max_center_y_drift * h;
        let horizontally_close = overlapping_x
            || (x_gap <= cfg.max_gap_to_height * h && center_gap <= cfg.max_center_y_drift * h);
        same_line_by_overlap && horizontally_close
    };

    for region in regs {
        let rect = region.rect;
        match group.last() {
            Some(last) if same_line(last, &rect) => {
                group.push(rect);
                group_conf.push(region.confidence);
                group_text.push(region.text);
            }
            _ => {
                if !group.is_empty() {
                    merged.push(make_merged(&group, &group_conf, &group_text));
                }
                group = vec![rect];
                group_conf = vec![region.confidence];
                group_text = vec![region.text];
            }
        }
    }
    if !group.is_empty() {
        merged.push(make_merged(&group, &group_conf, &group_text));
    }

    merged
}

fn union_of(group: &[RectF32]) -> RectF32 {
    let x0 = group.iter().map(|r| r.x0).fold(f32::INFINITY, f32::min);
    let y0 = group.iter().map(|r| r.y0).fold(f32::INFINITY, f32::min);
    let x1 = group.iter().map(|r| r.x1).fold(f32::NEG_INFINITY, f32::max);
    let y1 = group.iter().map(|r| r.y1).fold(f32::NEG_INFINITY, f32::max);
    RectF32::new(x0, y0, x1, y1)
}

fn make_merged(group: &[RectF32], conf: &[f32], text: &[Option<String>]) -> TextRegion {
    let rect = union_of(group);
    let confidence = conf.iter().copied().fold(0.0_f32, f32::max);
    let joined_text = if text.iter().all(|t| t.is_some()) {
        Some(
            text.iter()
                .map(|t| t.as_ref().cloned().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(" "),
        )
    } else {
        None
    };
    TextRegion {
        rect,
        confidence,
        text: joined_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> TextRegion {
        TextRegion::new(RectF32::new(x0, y0, x1, y1), 0.9)
    }

    #[test]
    fn filter_drops_below_threshold() {
        let regions = vec![
            TextRegion::new(RectF32::new(0.0, 0.0, 10.0, 10.0), 0.1),
            TextRegion::new(RectF32::new(0.0, 0.0, 10.0, 10.0), 0.5),
            TextRegion::new(RectF32::new(0.0, 0.0, 10.0, 10.0), 0.9),
        ];
        let out = filter_by_confidence(regions, 0.5);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|r| r.confidence >= 0.5));
    }

    #[test]
    fn merge_joins_words_on_same_line() {
        // "Hello   world   from   OCR"
        let regions = vec![
            r(0.0, 0.0, 30.0, 16.0),
            r(42.0, 0.0, 74.0, 16.0),
            r(86.0, 0.0, 106.0, 16.0),
            r(120.0, 0.0, 140.0, 16.0),
        ];
        let merged = merge_text_regions(regions, MergeConfig::default());
        assert_eq!(merged.len(), 1, "words on one line merge into a sentence");
        let b = merged[0].rect;
        assert_eq!(b.x0, 0.0);
        assert_eq!(b.x1, 140.0);
        assert_eq!(b.y0, 0.0);
        assert_eq!(b.y1, 16.0);
    }

    #[test]
    fn merge_keeps_separate_lines_separate() {
        // "Hello world" / "Second line"
        let regions = vec![r(0.0, 0.0, 70.0, 16.0), r(0.0, 20.0, 80.0, 36.0)];
        let merged = merge_text_regions(regions, MergeConfig::default());
        assert_eq!(merged.len(), 2, "lines must not merge");
    }

    #[test]
    fn merge_handles_partial_vertical_overlap() {
        // Slightly different baselines on the same line still merge.
        let regions = vec![r(0.0, 0.0, 40.0, 18.0), r(50.0, 2.0, 90.0, 20.0)];
        let merged = merge_text_regions(regions, MergeConfig::default());
        assert_eq!(merged.len(), 1);
        let b = merged[0].rect;
        assert_eq!(b.y0, 0.0);
        assert_eq!(b.y1, 20.0);
        assert_eq!(b.x1, 90.0);
    }

    #[test]
    fn merge_does_not_join_diagonal_offsets() {
        // Boxes offset in both X and Y are treated as separate elements.
        let regions = vec![r(0.0, 0.0, 30.0, 15.0), r(80.0, 40.0, 110.0, 55.0)];
        let merged = merge_text_regions(regions, MergeConfig::default());
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_large_gap_splits_groups() {
        // Two text runs far apart on the same row are kept separate.
        let regions = vec![r(0.0, 0.0, 50.0, 12.0), r(400.0, 0.0, 450.0, 12.0)];
        let merged = merge_text_regions(regions, MergeConfig::default());
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_thresholds_are_relative_to_height() {
        // Small fonts: a gap that would be fine for 10px text is huge for
        // 2px text, so they must stay separate.
        let small = vec![r(0.0, 0.0, 5.0, 2.0), r(20.0, 0.0, 30.0, 2.0)];
        let merged = merge_text_regions(small, MergeConfig::default());
        assert_eq!(merged.len(), 2, "gap/height ratio too big for tiny text");
    }

    #[test]
    fn union_makes_single_bounding_box() {
        let regions = vec![r(5.0, 3.0, 12.0, 9.0), r(6.0, 4.0, 15.0, 10.0)];
        let merged = merge_text_regions(regions, MergeConfig::default());
        assert_eq!(merged.len(), 1);
        let b = merged[0].rect;
        assert_eq!(b, RectF32::new(5.0, 3.0, 15.0, 10.0));
    }

    #[test]
    fn merge_propagates_max_confidence_and_joins_text() {
        let mut a = r(0.0, 0.0, 30.0, 16.0);
        a.confidence = 0.4;
        a.text = Some("Hello".into());
        let mut b = r(40.0, 0.0, 70.0, 16.0);
        b.confidence = 0.9;
        b.text = Some("world".into());
        let merged = merge_text_regions(vec![a, b], MergeConfig::default());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].confidence, 0.9);
        assert_eq!(merged[0].text.as_deref(), Some("Hello world"));
    }

    #[test]
    fn merge_without_text_keeps_none() {
        let a = TextRegion::new(RectF32::new(0.0, 0.0, 30.0, 16.0), 0.8);
        let b = TextRegion::new(RectF32::new(40.0, 0.0, 70.0, 16.0), 0.7);
        let merged = merge_text_regions(vec![a, b], MergeConfig::default());
        assert_eq!(merged[0].text, None);
    }
}
