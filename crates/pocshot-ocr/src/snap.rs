//! Convert detected text regions into the application's snap-guide format.
//!
//! OCR regions are grouped into text blocks and **only each block's outer
//! bounding box** contributes to the snapping lines: its top/bottom edges to
//! `horizontal` and its left/right edges to `vertical` (4 guides per block, no
//! per-line borders). Near-coincident guides between adjacent blocks are then
//! deduplicated.
//!
//! This type mirrors `pocshot_snap::SnapLines` so the GUI can hand the result
//! straight to its existing `combined_snap_lines()` / `snap_pos` machinery
//! without a second snapping implementation.

use crate::{RectF32, TextRegion};

/// Flattened snap-guide positions, reusing the same shape as
/// [`pocshot_snap::SnapLines`](https://docs.rs/pocshot-snap).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SnapGuides {
    pub horizontal: Vec<f32>,
    pub vertical: Vec<f32>,
}

/// Smallest region dimension that still contributes guides. Prevents a
/// handful of spurious 1px detector boxes from flooding the snapping engine.
const MIN_GUIDE_EXTENT: f32 = 3.0;

/// Adjacent border positions whose difference is no larger than this (image
/// px) are considered the same logical line and merge into one guide.
const MERGE_THRESHOLD: f32 = 2.0;

/// Minimum vertical gap (image px) allowed between OCR line-boxes before they
/// are considered separate blocks. The effective allowance grows with the line
/// height (see [`LINE_GAP_RATIO`]) so paragraphs set in a large font are not
/// split into one block per line.
const BLOCK_GAP: f32 = 8.0;

/// Vertical gap allowance between adjacent line-boxes, as a fraction of the
/// smaller box height. Line spacing scales with font size, so a fixed pixel gap
/// splits large text into per-line blocks (4 guides each) while a
/// height-relative gap keeps each paragraph a single block.
const LINE_GAP_RATIO: f32 = 0.7;

/// Required horizontal overlap between a line-box and a block, as a fraction of
/// the smaller width. Prevents two side-by-side columns (or an unrelated UI
/// element that merely touches an edge) from chaining into one block.
const H_OVERLAP_RATIO: f32 = 0.3;

/// Deduplicate near-coincident positions.
///
/// - Sorts `values` ascending.
/// - Walks the sorted list and groups each consecutive value into the current
///   cluster when its difference from the **previous** value is ≤
///   `merge_threshold`. This is *compare-to-previous* chaining: borders at
///   10.0, 11.5, 13.0 with a threshold of 2.0 all merge into one cluster
///   (10→11.5 and 11.5→13 are each within tolerance) even though the first and
///   last, 10.0 and 13.0, are farther apart than the threshold. That is the
///   behaviour wanted here: the chained run reflects genuinely contiguous text
///   lines. (Compare-to-cluster-start would instead split 13.0 off, which is
///   not desired.)
/// - Each cluster collapses to a single representative — the arithmetic **mean**
///   of its values.
///
/// Output is sorted ascending and never longer than the input.
fn merge_lines(values: &[f32], merge_threshold: f32) -> Vec<f32> {
    let mut sorted: Vec<f32> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if sorted.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::with_capacity(sorted.len());
    let mut cluster_sum = sorted[0];
    let mut cluster_count = 1usize;
    let mut cluster_last = sorted[0];

    for &value in &sorted[1..] {
        if value - cluster_last <= merge_threshold {
            // Chain into the current cluster (compare-to-previous).
            cluster_sum += value;
            cluster_count += 1;
            cluster_last = value;
        } else {
            merged.push(cluster_sum / cluster_count as f32);
            cluster_sum = value;
            cluster_count = 1;
            cluster_last = value;
        }
    }
    merged.push(cluster_sum / cluster_count as f32);
    merged
}

/// Group OCR line-boxes into text blocks (paragraphs): a region joins the
/// existing block it overlaps best when its vertical gap is within the
/// (height-relative) allowance and its horizontal overlap is significant.
/// Picking the best candidate instead of the first avoids greedy chaining
/// through a block that only touches at a corner; requiring a real horizontal
/// overlap keeps side-by-side columns apart. Each block becomes one bounding
/// box; the returned boxes are clamped to the image.
fn group_into_blocks(regions: &[TextRegion], image_w: f32, image_h: f32) -> Vec<RectF32> {
    let mut rects: Vec<RectF32> = regions
        .iter()
        .filter(|r| r.rect.width() >= MIN_GUIDE_EXTENT && r.rect.height() >= MIN_GUIDE_EXTENT)
        .map(|r| RectF32 {
            x0: r.rect.x0.clamp(0.0, image_w),
            y0: r.rect.y0.clamp(0.0, image_h),
            x1: r.rect.x1.clamp(0.0, image_w),
            y1: r.rect.y1.clamp(0.0, image_h),
        })
        .collect();
    rects.sort_by(|a, b| {
        a.y0.partial_cmp(&b.y0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut blocks: Vec<RectF32> = Vec::new();
    for r in rects {
        let mut best: Option<(usize, f32)> = None;
        for (i, b) in blocks.iter().enumerate() {
            let vertical_gap = (r.y0 - b.y1).max(b.y0 - r.y1).max(0.0);
            let gap_limit = BLOCK_GAP.max(LINE_GAP_RATIO * r.height().min(b.height()));
            if vertical_gap > gap_limit {
                continue;
            }
            let overlap = (r.x1.min(b.x1) - r.x0.max(b.x0)).max(0.0);
            let smaller_w = r.width().min(b.width());
            if smaller_w <= 0.0 {
                continue;
            }
            let ratio = overlap / smaller_w;
            if ratio < H_OVERLAP_RATIO {
                continue;
            }
            if best.is_none_or(|(_, best_ratio)| ratio > best_ratio) {
                best = Some((i, ratio));
            }
        }

        match best {
            Some((i, _)) => {
                let b = &mut blocks[i];
                b.x0 = b.x0.min(r.x0);
                b.y0 = b.y0.min(r.y0);
                b.x1 = b.x1.max(r.x1);
                b.y1 = b.y1.max(r.y1);
            }
            None => blocks.push(r),
        }
    }
    blocks
}

/// Build snap guides for the given OCR regions (expected in original-image
/// coordinates, clamped to `(image_w, image_h)`).
///
/// Regions are first grouped into text blocks (see [`group_into_blocks`]), and
/// **only each block's outer bounding box** generates guides — its top and
/// bottom edges to `horizontal`, its left and right edges to `vertical` — 4
/// guides per block regardless of how many lines are in it. Guides from
/// different blocks are then deduplicated via [`merge_lines`] so a seam shared
/// between two tightly-adjacent blocks yields one line instead of two.
pub fn text_regions_to_guides(regions: &[TextRegion], image_w: f32, image_h: f32) -> SnapGuides {
    let blocks = group_into_blocks(regions, image_w, image_h);
    let mut horizontal = Vec::with_capacity(blocks.len() * 2);
    let mut vertical = Vec::with_capacity(blocks.len() * 2);

    for b in &blocks {
        vertical.push(b.x0);
        vertical.push(b.x1);
        horizontal.push(b.y0);
        horizontal.push(b.y1);
    }

    SnapGuides {
        horizontal: merge_lines(&horizontal, MERGE_THRESHOLD),
        vertical: merge_lines(&vertical, MERGE_THRESHOLD),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RectF32, TextRegion};

    #[test]
    fn single_region_yields_its_four_edges() {
        let region = TextRegion::new(RectF32::new(100.0, 200.0, 300.0, 400.0), 0.9);
        let guides = text_regions_to_guides(&[region], 1920.0, 1080.0);
        assert_eq!(guides.vertical, vec![100.0, 300.0]);
        assert_eq!(guides.horizontal, vec![200.0, 400.0]);
    }

    #[test]
    fn multiline_block_yields_only_its_outer_box() {
        // Four tightly-stacked lines (gaps ≤ BLOCK_GAP) group into ONE block;
        // only the outer bounding box generates guides: 2 horizontal + 2 vertical.
        let lines = [
            RectF32::new(100.0, 100.0, 300.0, 150.0),
            RectF32::new(100.0, 151.0, 300.0, 201.0),
            RectF32::new(100.0, 202.0, 300.0, 252.0),
            RectF32::new(100.0, 253.0, 300.0, 303.0),
        ];
        let regions: Vec<TextRegion> = lines.iter().map(|r| TextRegion::new(*r, 0.9)).collect();
        let guides = text_regions_to_guides(&regions, 1920.0, 1080.0);
        assert_eq!(guides.horizontal, vec![100.0, 303.0]);
        assert_eq!(guides.vertical, vec![100.0, 300.0]);
        assert_eq!(guides.horizontal.len(), 2);
        assert_eq!(guides.vertical.len(), 2);
    }

    #[test]
    fn side_by_side_columns_stay_separate_blocks() {
        // Same vertical span but disjoint x: no horizontal overlap → 2 blocks.
        let a = TextRegion::new(RectF32::new(0.0, 0.0, 50.0, 50.0), 0.9);
        let b = TextRegion::new(RectF32::new(60.0, 0.0, 110.0, 50.0), 0.9);
        let guides = text_regions_to_guides(&[a, b], 1920.0, 1080.0);
        // Two blocks share the same top/bottom → merged horizontal pair.
        assert_eq!(guides.horizontal, vec![0.0, 50.0]);
        // Left/right edges of both blocks remain.
        assert_eq!(guides.vertical, vec![0.0, 50.0, 60.0, 110.0]);
    }

    #[test]
    fn distant_blocks_stay_separate() {
        // Gap of 60px exceeds the height-relative allowance for 50px-tall boxes
        // (max(8, 0.7*50) = 35) → two blocks with no shared edge to merge.
        let a = TextRegion::new(RectF32::new(0.0, 100.0, 100.0, 150.0), 0.9);
        let b = TextRegion::new(RectF32::new(0.0, 210.0, 100.0, 260.0), 0.9);
        let guides = text_regions_to_guides(&[a, b], 1920.0, 1080.0);
        assert_eq!(guides.horizontal, vec![100.0, 150.0, 210.0, 260.0]);
        // Both blocks share left/right edges → merged vertical pair.
        assert_eq!(guides.vertical, vec![0.0, 100.0]);
    }

    #[test]
    fn large_font_line_spacing_stays_one_block() {
        // 40px-tall lines spaced 20px apart: the fixed 8px gap would split them,
        // but the height-relative allowance (0.7*40 = 28) keeps one block, so a
        // paragraph contributes only its outer borders.
        let lines = [
            RectF32::new(100.0, 100.0, 400.0, 140.0),
            RectF32::new(100.0, 160.0, 400.0, 200.0),
            RectF32::new(100.0, 220.0, 400.0, 260.0),
        ];
        let regions: Vec<TextRegion> = lines.iter().map(|r| TextRegion::new(*r, 0.9)).collect();
        let guides = text_regions_to_guides(&regions, 1920.0, 1080.0);
        assert_eq!(guides.horizontal, vec![100.0, 260.0]);
        assert_eq!(guides.vertical, vec![100.0, 400.0]);
    }

    #[test]
    fn columns_do_not_chain_through_small_overlap() {
        // Left column block (x 0..100) and a right-column line that only clips
        // its edge (5px overlap = 5% < 30%) must stay separate blocks.
        let left = TextRegion::new(RectF32::new(0.0, 0.0, 100.0, 40.0), 0.9);
        let left2 = TextRegion::new(RectF32::new(0.0, 42.0, 100.0, 80.0), 0.9);
        let right = TextRegion::new(RectF32::new(95.0, 44.0, 195.0, 84.0), 0.9);
        let guides = text_regions_to_guides(&[left, left2, right], 1920.0, 1080.0);
        // Left lines group into one block (y 0..80); the right line is its own.
        assert!(guides.horizontal.contains(&0.0));
        assert!(guides.horizontal.contains(&80.0));
        assert!(guides.horizontal.contains(&44.0));
        assert!(guides.horizontal.contains(&84.0));
    }

    #[test]
    fn small_font_with_large_gap_stays_separate() {
        // Tiny 8px lines 20px apart: allowance is max(8, 5.6) = 8 → split.
        let a = TextRegion::new(RectF32::new(0.0, 0.0, 60.0, 8.0), 0.9);
        let b = TextRegion::new(RectF32::new(0.0, 28.0, 60.0, 36.0), 0.9);
        let guides = text_regions_to_guides(&[a, b], 1920.0, 1080.0);
        assert_eq!(guides.horizontal, vec![0.0, 8.0, 28.0, 36.0]);
    }

    #[test]
    fn chain_within_threshold_merges_even_when_ends_are_far_apart() {
        // 10->11.5 and 11.5->13 are each within 2.0, but 10->13 (3.0) is not;
        // compare-to-previous chaining must still merge all three.
        assert_eq!(merge_lines(&[10.0, 11.5, 13.0], 2.0), vec![11.5]);
        let merged = merge_lines(&[51.0, 52.0, 52.5, 60.0], 2.0);
        assert_eq!(merged.len(), 2);
        // Cluster [51, 52, 52.5] -> mean ≈ 51.8333; compare with tolerance.
        assert!((merged[0] - 51.8333).abs() < 1e-3, "got {}", merged[0]);
        assert_eq!(merged[1], 60.0);
    }

    #[test]
    fn merge_lines_empty_and_single_inputs() {
        assert_eq!(merge_lines(&[], 2.0), Vec::<f32>::new());
        assert_eq!(merge_lines(&[5.0], 2.0), vec![5.0]);
        // Large gaps keep everything distinct.
        assert_eq!(
            merge_lines(&[1.0, 100.0, 200.0], 2.0),
            vec![1.0, 100.0, 200.0]
        );
        // Output is sorted ascending regardless of input order.
        assert_eq!(merge_lines(&[40.0, 10.0, 11.0], 2.0), vec![10.5, 40.0]);
    }

    #[test]
    fn tiny_regions_generate_no_guides() {
        let region = TextRegion::new(RectF32::new(1.0, 1.0, 2.0, 2.0), 0.9);
        let guides = text_regions_to_guides(&[region], 1920.0, 1080.0);
        assert!(guides.vertical.is_empty());
        assert!(guides.horizontal.is_empty());
    }

    #[test]
    fn guides_are_clamped_to_image_bounds() {
        let region = TextRegion::new(RectF32::new(-10.0, -20.0, 2000.0, 2000.0), 0.9);
        let guides = text_regions_to_guides(&[region], 1920.0, 1080.0);
        assert!(guides.vertical.iter().all(|&x| (0.0..=1920.0).contains(&x)));
        assert!(guides
            .horizontal
            .iter()
            .all(|&y| (0.0..=1080.0).contains(&y)));
    }
}
