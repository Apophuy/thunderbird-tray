// SPDX-License-Identifier: GPL-3.0-only

//! Self-contained, HiDPI-ready ARGB tray artwork.

use std::sync::LazyLock;

use ksni::Icon;

use crate::tray::TrayIcon;

const SIZES: [i32; 3] = [22, 32, 48];
const SUPERSAMPLING: i32 = 4;

static CONNECTED: LazyLock<Vec<Icon>> = LazyLock::new(|| render_family(TrayIcon::Connected));
static UNREAD: LazyLock<Vec<Icon>> = LazyLock::new(|| render_family(TrayIcon::Unread));
static DISCONNECTED: LazyLock<Vec<Icon>> = LazyLock::new(|| render_family(TrayIcon::Disconnected));

pub fn pixmaps(icon: TrayIcon) -> Vec<Icon> {
    match icon {
        TrayIcon::Connected => CONNECTED.clone(),
        TrayIcon::Unread => UNREAD.clone(),
        TrayIcon::Disconnected => DISCONNECTED.clone(),
    }
}

fn render_family(state: TrayIcon) -> Vec<Icon> {
    SIZES.into_iter().map(|size| render(state, size)).collect()
}

fn render(state: TrayIcon, size: i32) -> Icon {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let scale = f64::from(size) / 32.0;

    for y in 0..size {
        for x in 0..size {
            let mut color_totals = [0_u32; 3];
            let mut opaque_samples = 0_u32;
            for sample_y in 0..SUPERSAMPLING {
                for sample_x in 0..SUPERSAMPLING {
                    let px = (f64::from(x) + (f64::from(sample_x) + 0.5) / 4.0) / scale;
                    let py = (f64::from(y) + (f64::from(sample_y) + 0.5) / 4.0) / scale;
                    let color = sample(state, px, py);
                    if color[0] != 0 {
                        opaque_samples += 1;
                        for (total, channel) in color_totals.iter_mut().zip(&color[1..]) {
                            *total += u32::from(*channel);
                        }
                    }
                }
            }
            let samples = (SUPERSAMPLING * SUPERSAMPLING) as u32;
            data.push(((opaque_samples * 255) / samples) as u8);
            for total in color_totals {
                data.push(total.checked_div(opaque_samples).unwrap_or(0) as u8);
            }
        }
    }

    Icon {
        width: size,
        height: size,
        data,
    }
}

fn sample(state: TrayIcon, x: f64, y: f64) -> [u8; 4] {
    let mut color = [0, 0, 0, 0];
    let body_color = match state {
        TrayIcon::Disconnected => [255, 112, 122, 137],
        TrayIcon::Connected | TrayIcon::Unread => [255, 42, 105, 176],
    };

    if inside_rounded_rect(x, y, 2.5, 5.5, 29.5, 26.5, 3.0) {
        color = body_color;
        let left_flap = distance_to_segment(x, y, 4.5, 8.0, 16.0, 17.0);
        let right_flap = distance_to_segment(x, y, 27.5, 8.0, 16.0, 17.0);
        if left_flap <= 1.15 || right_flap <= 1.15 {
            color = [255, 236, 244, 255];
        }
    }

    if state == TrayIcon::Unread {
        let badge_distance = ((x - 25.0).powi(2) + (y - 7.0).powi(2)).sqrt();
        if badge_distance <= 6.3 {
            color = [255, 247, 249, 252];
        }
        if badge_distance <= 4.8 {
            color = [255, 239, 101, 89];
        }
    }

    if state == TrayIcon::Disconnected {
        let slash = distance_to_segment(x, y, 6.0, 4.5, 27.0, 27.0);
        if slash <= 2.7 {
            color = [255, 247, 249, 252];
        }
        if slash <= 1.65 {
            color = [255, 220, 62, 65];
        }
    }

    color
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
