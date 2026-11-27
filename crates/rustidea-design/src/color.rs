//! Small, renderer-independent colour type.

use serde::{Deserialize, Serialize};

/// An sRGB colour with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0.0);
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Parse the concrete forms the token generator emits: `#rgb`, `#rrggbb`,
    /// `#rrggbbaa`, `rgb(...)` and `rgba(...)`.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value == "transparent" {
            return Some(Self::TRANSPARENT);
        }
        if let Some(hex) = value.strip_prefix('#') {
            return Self::parse_hex(hex);
        }
        let inner = value
            .strip_prefix("rgba(")
            .or_else(|| value.strip_prefix("rgb("))?
            .strip_suffix(')')?;
        let normalized = inner.replace('/', " ");
        let parts: Vec<&str> = normalized
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let channel = |p: &str| -> Option<u8> {
            if let Some(p) = p.strip_suffix('%') {
                Some((p.parse::<f32>().ok()? * 2.55).round().clamp(0.0, 255.0) as u8)
            } else {
                Some(p.parse::<f32>().ok()?.clamp(0.0, 255.0) as u8)
            }
        };
        let a: f32 = match parts.get(3) {
            None => 1.0,
            Some(p) => {
                if let Some(p) = p.strip_suffix('%') {
                    p.parse::<f32>().ok()? / 100.0
                } else {
                    p.parse::<f32>().ok()?
                }
            }
        };
        Some(Self::rgba(
            channel(parts[0])?,
            channel(parts[1])?,
            channel(parts[2])?,
            a.clamp(0.0, 1.0),
        ))
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim();
        let full = match hex.len() {
            3 | 4 => {
                let mut full = String::with_capacity(8);
                for ch in hex.chars() {
                    full.push(ch);
                    full.push(ch);
                }
                full
            }
            6 | 8 => hex.to_owned(),
            _ => return None,
        };
        let byte = |range: std::ops::Range<usize>| u8::from_str_radix(&full[range], 16).ok();
        let a = if full.len() == 8 {
            byte(6..8)? as f32 / 255.0
        } else {
            1.0
        };
        Some(Self::rgba(byte(0..2)?, byte(2..4)?, byte(4..6)?, a))
    }

    /// HSLA in GPUI's convention (h/s/l in 0..=1).
    pub fn to_hsla(self) -> (f32, f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let delta = max - min;
        if delta <= f32::EPSILON {
            return (0.0, 0.0, l, self.a);
        }
        let s = if l > 0.5 {
            delta / (2.0 - max - min)
        } else {
            delta / (max + min)
        };
        let mut h = if max == r {
            (g - b) / delta + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        h /= 6.0;
        (h, s, l, self.a)
    }

    /// Composite `self` over `background` using straight-alpha source-over.
    pub fn over(self, background: Self) -> Self {
        let a = self.a + background.a * (1.0 - self.a);
        if a <= f32::EPSILON {
            return Self::TRANSPARENT;
        }
        let mix = |fg: u8, bg: u8| {
            (((fg as f32 * self.a) + (bg as f32 * background.a * (1.0 - self.a))) / a)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Self::rgba(
            mix(self.r, background.r),
            mix(self.g, background.g),
            mix(self.b, background.b),
            a,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generated_colour_forms() {
        assert_eq!(Color::parse("#3871e1"), Some(Color::rgb(0x38, 0x71, 0xe1)));
        assert_eq!(Color::parse("#fff"), Some(Color::WHITE));
        let translucent = Color::parse("rgba(18, 52, 86, .5)").unwrap();
        assert_eq!((translucent.r, translucent.g, translucent.b), (18, 52, 86));
        assert_eq!(translucent.a, 0.5);
        assert_eq!(Color::parse("transparent"), Some(Color::TRANSPARENT));
    }

    #[test]
    fn hsla_round_trips_primary_channels() {
        let (h, s, l, a) = Color::rgb(255, 0, 0).to_hsla();
        assert!((h - 0.0).abs() < 1e-6);
        assert!((s - 1.0).abs() < 1e-6);
        assert!((l - 0.5).abs() < 1e-6);
        assert_eq!(a, 1.0);
    }

    #[test]
    fn composites_alpha_over_opaque_background() {
        let result = Color::rgba(255, 255, 255, 0.5).over(Color::BLACK);
        assert_eq!((result.r, result.g, result.b), (128, 128, 128));
        assert_eq!(result.a, 1.0);
    }
}
