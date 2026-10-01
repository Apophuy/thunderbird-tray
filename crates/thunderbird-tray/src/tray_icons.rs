// SPDX-License-Identifier: GPL-3.0-only

//! Embedded pseudo-3D artwork and HiDPI-ready SNI ARGB rendering.

use std::sync::LazyLock;

use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use ksni::Icon;

use crate::tray::TrayIcon;

const SIZES: [i32; 3] = [22, 32, 48];
const SUPERSAMPLING: i32 = 4;
const LIGHT: [u8; 3] = [255, 247, 237];
const UNREAD_RED: [u8; 3] = [185, 28, 28];

const CONNECTED_PNG: &[u8] = include_bytes!("../../../assets/thunderbird-tray-amber.png");
const UNREAD_PNG: &[u8] = include_bytes!("../../../assets/thunderbird-tray-unread.png");

static CONNECTED_MASTER: LazyLock<Result<RgbaImage, String>> =
    LazyLock::new(|| decode_master(CONNECTED_PNG));
static UNREAD_MASTER: LazyLock<Result<RgbaImage, String>> =
    LazyLock::new(|| decode_master(UNREAD_PNG));
static CONNECTED: LazyLock<Vec<Icon>> = LazyLock::new(|| render_family(TrayIcon::Connected, None));
static DISCONNECTED: LazyLock<Vec<Icon>> =
    LazyLock::new(|| render_family(TrayIcon::Disconnected, None));

pub fn pixmaps(icon: TrayIcon, unread_count: Option<u32>) -> Vec<Icon> {
    match icon {
        TrayIcon::Connected => CONNECTED.clone(),
        TrayIcon::Unread => render_family(TrayIcon::Unread, unread_count),
        TrayIcon::Disconnected => DISCONNECTED.clone(),
    }
}

pub fn application_image() -> Option<slint::Image> {
    let image = CONNECTED_MASTER.as_ref().ok()?;
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
    );
    Some(slint::Image::from_rgba8(buffer))
}

fn decode_master(bytes: &[u8]) -> Result<RgbaImage, String> {
    image::load_from_memory(bytes)
        .map(DynamicImage::into_rgba8)
        .map_err(|error| format!("invalid embedded tray artwork: {error}"))
}

fn render_family(state: TrayIcon, unread_count: Option<u32>) -> Vec<Icon> {
    SIZES
        .into_iter()
        .map(|size| render(state, unread_count, size))
        .collect()
}

fn render(state: TrayIcon, unread_count: Option<u32>, size: i32) -> Icon {
    let master = match state {
        TrayIcon::Unread => &*UNREAD_MASTER,
        TrayIcon::Connected | TrayIcon::Disconnected => &*CONNECTED_MASTER,
    };
    let master = match master {
        Ok(master) => master,
        Err(error) => {
            tracing::error!(error, "could not decode embedded tray artwork");
            return Icon {
                width: size,
                height: size,
                data: vec![0; (size * size * 4) as usize],
            };
        }
    };
    let mut resized =
        image::imageops::resize(master, size as u32, size as u32, FilterType::Lanczos3);

    if state == TrayIcon::Disconnected {
        for pixel in resized.pixels_mut() {
            let luminance =
                (u16::from(pixel[0]) * 54 + u16::from(pixel[1]) * 183 + u16::from(pixel[2]) * 19)
                    / 256;
            let muted = ((luminance * 3) / 4) as u8;
            pixel[0] = muted;
            pixel[1] = muted;
            pixel[2] = muted;
        }
    }

    let scale = f64::from(size) / 32.0;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut base = resized.get_pixel(x as u32, y as u32).0;
            if let Some((overlay, coverage)) = overlay_pixel(state, unread_count, x, y, scale) {
                base = blend(base, overlay, coverage);
            }
            data.extend_from_slice(&[base[3], base[0], base[1], base[2]]);
        }
    }

    Icon {
        width: size,
        height: size,
        data,
    }
}

fn overlay_pixel(
    state: TrayIcon,
    unread_count: Option<u32>,
    x: i32,
    y: i32,
    scale: f64,
) -> Option<([u8; 3], u8)> {
    let mut totals = [0_u32; 3];
    let mut covered = 0_u32;
    for sample_y in 0..SUPERSAMPLING {
        for sample_x in 0..SUPERSAMPLING {
            let px = (f64::from(x) + (f64::from(sample_x) + 0.5) / 4.0) / scale;
            let py = (f64::from(y) + (f64::from(sample_y) + 0.5) / 4.0) / scale;
            if let Some(color) = overlay_sample(state, unread_count, px, py) {
                covered += 1;
                for (total, channel) in totals.iter_mut().zip(color) {
                    *total += u32::from(channel);
                }
            }
        }
    }
    if covered == 0 {
        return None;
    }
    let color = totals.map(|total| (total / covered) as u8);
    let samples = (SUPERSAMPLING * SUPERSAMPLING) as u32;
    Some((color, ((covered * 255) / samples) as u8))
}

fn overlay_sample(state: TrayIcon, unread_count: Option<u32>, x: f64, y: f64) -> Option<[u8; 3]> {
    if state == TrayIcon::Unread {
        let text = unread_count.map(badge_text);
        let mut color = None;
        if inside_rounded_rect(x, y, 17.7, 0.5, 31.5, 13.5, 6.5) {
            color = Some(LIGHT);
        }
        if inside_rounded_rect(x, y, 18.9, 1.7, 30.3, 12.3, 5.3) {
            color = Some(UNREAD_RED);
        }
        if text.is_some_and(|text| glyph_text_contains(&text, x, y)) {
            color = Some(LIGHT);
        }
        return color;
    }

    if state == TrayIcon::Disconnected {
        let slash = distance_to_segment(x, y, 6.0, 4.5, 27.0, 27.0);
        if slash <= 1.65 {
            return Some(UNREAD_RED);
        }
        if slash <= 2.7 {
            return Some(LIGHT);
        }
    }
    None
}

fn blend(base: [u8; 4], overlay: [u8; 3], coverage: u8) -> [u8; 4] {
    let foreground_alpha = u32::from(coverage);
    let background_alpha = u32::from(base[3]);
    let output_alpha = foreground_alpha + (background_alpha * (255 - foreground_alpha)) / 255;
    if output_alpha == 0 {
        return [0; 4];
    }
    let mut result = [0_u8; 4];
    for channel in 0..3 {
        let foreground = u32::from(overlay[channel]) * foreground_alpha;
        let background =
            u32::from(base[channel]) * background_alpha * (255 - foreground_alpha) / 255;
        result[channel] = ((foreground + background) / output_alpha) as u8;
    }
    result[3] = output_alpha as u8;
    result
}

fn badge_text(count: u32) -> String {
    if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    }
}

fn glyph_text_contains(text: &str, x: f64, y: f64) -> bool {
    let scale = match text.len() {
        1 => 1.35,
        2 => 1.05,
        _ => 0.82,
    };
    let glyph_width = 3.0 * scale;
    let gap = scale;
    let text_width = glyph_width * text.len() as f64 + gap * text.len().saturating_sub(1) as f64;
    let left = 24.6 - text_width / 2.0;
    let top = 7.0 - 2.5 * scale;

    text.chars().enumerate().any(|(index, character)| {
        let glyph_left = left + index as f64 * (glyph_width + gap);
        let column = ((x - glyph_left) / scale).floor() as i32;
        let row = ((y - top) / scale).floor() as i32;
        (0..3).contains(&column)
            && (0..5).contains(&row)
            && glyph_rows(character)[row as usize] & (1 << (2 - column)) != 0
    })
}

fn glyph_rows(character: char) -> [u8; 5] {
    match character {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        _ => [0; 5],
    }
}

fn inside_rounded_rect(
    x: f64,
    y: f64,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    radius: f64,
) -> bool {
    let closest_x = x.clamp(left + radius, right - radius);
    let closest_y = y.clamp(top + radius, bottom - radius);
    (x - closest_x).powi(2) + (y - closest_y).powi(2) <= radius.powi(2)
}

fn distance_to_segment(x: f64, y: f64, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> f64 {
    let length_squared = (end_x - start_x).powi(2) + (end_y - start_y).powi(2);
    let projection = (((x - start_x) * (end_x - start_x) + (y - start_y) * (end_y - start_y))
        / length_squared)
        .clamp(0.0, 1.0);
    let projected_x = start_x + projection * (end_x - start_x);
    let projected_y = start_y + projection * (end_y - start_y);
    ((x - projected_x).powi(2) + (y - projected_y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_masters_are_square_transparent_and_visually_distinct() {
        let connected = CONNECTED_MASTER.as_ref().unwrap();
        let unread = UNREAD_MASTER.as_ref().unwrap();
        assert_eq!(connected.dimensions(), (1254, 1254));
        assert_eq!(unread.dimensions(), connected.dimensions());
        assert_eq!(connected.get_pixel(0, 0)[3], 0);
        assert_eq!(unread.get_pixel(0, 0)[3], 0);
        assert_ne!(connected.as_raw(), unread.as_raw());
    }

    #[test]
    fn connected_is_amber_and_unread_contains_a_red_envelope() {
        let connected = render(TrayIcon::Connected, None, 48);
        let unread = render(TrayIcon::Unread, Some(7), 48);
        assert!(connected.data.chunks_exact(4).any(|pixel| {
            pixel[0] > 200 && pixel[1] > 180 && pixel[2] > 80 && pixel[2] < 220 && pixel[3] < 100
        }));
        assert!(
            unread.data.chunks_exact(4).any(|pixel| {
                pixel[0] > 200 && pixel[1] > 180 && pixel[2] < 80 && pixel[3] < 80
            })
        );
    }

    #[test]
    fn badge_text_preserves_small_counts_and_bounds_large_counts() {
        assert_eq!(badge_text(1), "1");
        assert_eq!(badge_text(42), "42");
        assert_eq!(badge_text(99), "99");
        assert_eq!(badge_text(100), "99+");
        assert_eq!(badge_text(u32::MAX), "99+");
    }

    #[test]
    fn one_two_and_overflow_badges_render_distinct_glyphs() {
        let one = render(TrayIcon::Unread, Some(1), 32);
        let twelve = render(TrayIcon::Unread, Some(12), 32);
        let overflow = render(TrayIcon::Unread, Some(100), 32);
        assert_ne!(one.data, twelve.data);
        assert_ne!(twelve.data, overflow.data);
        assert_ne!(one.data, overflow.data);
    }
}
