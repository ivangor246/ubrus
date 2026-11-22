use crate::library::{Profile, Store};
use iced::widget::image;

pub(super) const TILE_HEIGHT: f32 = 124.;
pub(super) const GAP: f32 = 10.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Page {
    pub columns: usize,
    pub first: usize,
    pub end: usize,
    pub before: f32,
    pub after: f32,
}

pub(super) fn page(width: f32, height: f32, offset: f32, count: usize) -> Page {
    let columns = ((width + GAP) / (186. + GAP)).ceil().max(1.) as usize;
    let rows = count.div_ceil(columns);
    let stride = TILE_HEIGHT + GAP;
    let visible_rows = (height / stride).ceil() as usize + 1;
    let top = ((offset / stride) as usize).min(rows.saturating_sub(visible_rows));
    let first = top.saturating_sub(1);
    let end = (top + visible_rows + 1).min(rows);
    Page {
        columns,
        first: first * columns,
        end: (end * columns).min(count),
        before: first as f32 * stride,
        after: (rows - end) as f32 * stride,
    }
}

/// Move by complete rows so consecutive pages overlap when the last row is clipped.
pub(super) fn keyboard_page(
    columns: usize,
    count: usize,
    viewport_height: f32,
    offset: f32,
    forward: bool,
) -> Option<(f32, usize)> {
    if count == 0 {
        return None;
    }
    let stride = TILE_HEIGHT + GAP;
    let rows = count.div_ceil(columns);
    let step = (viewport_height / stride).floor().max(1.) * stride;
    let max = (rows as f32 * stride - GAP - viewport_height).max(0.);
    let next = (if forward { offset + step } else { offset - step }).clamp(0., max);
    let first_row = ((next / stride).ceil() as usize).min(rows - 1);
    Some((next, first_row * columns))
}

pub(super) fn matches(profile: &Profile, sorted: &[String], query: &str) -> Vec<String> {
    let query = query.trim().to_lowercase();
    sorted
        .iter()
        .filter(|id| query.is_empty() || profile.items[*id].name.to_lowercase().contains(&query))
        .cloned()
        .collect()
}

pub(super) fn thumbnail(store: &Store, id: &str) -> Option<image::Handle> {
    let mut reader = ::image::ImageReader::open(store.thumb(id)).ok()?;
    let mut limits = ::image::Limits::default();
    // Imported posters are at most 384×256, with either orientation after rotation.
    limits.max_image_width = Some(384);
    limits.max_image_height = Some(384);
    reader.limits(limits);
    let pixels = reader.decode().ok()?.into_rgba8();
    Some(image::Handle::from_rgba(
        pixels.width(),
        pixels.height(),
        pixels.into_raw(),
    ))
}
