#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorLiteral {
    pub start: usize,
    pub end: usize,
    pub rgba: [u8; 4],
}

pub fn find_color_literals(text: &str) -> Vec<ColorLiteral> {
    let mut colors = find_hex_colors(text);
    colors.extend(find_function_colors(text));
    colors.sort_by_key(|color| color.start);
    colors
}

fn find_hex_colors(text: &str) -> Vec<ColorLiteral> {
    let bytes = text.as_bytes();
    let mut colors = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#' || previous_is_word(bytes, index) {
            index += 1;
            continue;
        }
        let mut matched = None;
        for digits in [8, 6, 4, 3] {
            let end = index + 1 + digits;
            if end > bytes.len() {
                continue;
            }
            let candidate = &bytes[index + 1..end];
            if !candidate.iter().all(|byte| byte.is_ascii_hexdigit()) {
                continue;
            }
            if end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                continue;
            }
            matched = Some((end, digits));
            break;
        }
        let Some((end, digits)) = matched else {
            index += 1;
            continue;
        };
        if let Some(rgba) = parse_hex(&text[index + 1..end], digits) {
            colors.push(ColorLiteral {
                start: index,
                end,
                rgba,
            });
            index = end;
        } else {
            index += 1;
        }
    }
    colors
}

fn previous_is_word(bytes: &[u8], index: usize) -> bool {
    index > 0 && (bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_')
}

fn parse_hex(raw: &str, digits: usize) -> Option<[u8; 4]> {
    let mut channels = [0u8; 4];
    match digits {
        3 | 4 => {
            for (index, byte) in raw.bytes().enumerate() {
                let value = (byte as char).to_digit(16)? as u8;
                channels[index] = value * 17;
            }
            if digits == 3 {
                channels[3] = 255;
            }
        }
        6 | 8 => {
            for index in 0..digits / 2 {
                let value = u8::from_str_radix(&raw[index * 2..index * 2 + 2], 16).ok()?;
                channels[index] = value;
            }
            if digits == 6 {
                channels[3] = 255;
            }
        }
        _ => return None,
    }
    Some(channels)
}

fn find_function_colors(text: &str) -> Vec<ColorLiteral> {
    let bytes = text.as_bytes();
    let mut colors = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_alphabetic() || previous_is_word(bytes, index) {
            index += 1;
            continue;
        }
        let Some((name, end, rgba)) = parse_function_at(text, index) else {
            index += 1;
            continue;
        };
        colors.push(ColorLiteral {
            start: index,
            end,
            rgba,
        });
        index = end.max(index + name.len());
    }
    colors
}

fn parse_function_at(text: &str, index: usize) -> Option<(&'static str, usize, [u8; 4])> {
    for name in ["rgba", "rgb", "hsla", "hsl"] {
        let rest = text.get(index..)?;
        if !rest.starts_with(name) {
            continue;
        }
        if rest
            .as_bytes()
            .get(name.len())
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            continue;
        }
        let mut cursor = index + name.len();
        while text
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        if text.as_bytes().get(cursor) != Some(&b'(') {
            continue;
        }
        let open = cursor + 1;
        let close = text.get(open..)?.find(')')? + open;
        let args = text.get(open..close)?;
        let channels = args
            .split(|character: char| {
                character == ',' || character == '/' || character.is_whitespace()
            })
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let rgba = match name {
            "rgb" if channels.len() == 3 => {
                let (red, green, blue) = (
                    parse_rgb_channel(channels[0])?,
                    parse_rgb_channel(channels[1])?,
                    parse_rgb_channel(channels[2])?,
                );
                [red, green, blue, 255]
            }
            "rgba" if channels.len() == 4 => {
                let (red, green, blue) = (
                    parse_rgb_channel(channels[0])?,
                    parse_rgb_channel(channels[1])?,
                    parse_rgb_channel(channels[2])?,
                );
                [red, green, blue, parse_alpha(channels[3])?]
            }
            "hsl" if channels.len() == 3 => {
                let rgb = hsl_to_rgb(channels[0], channels[1], channels[2])?;
                [rgb[0], rgb[1], rgb[2], 255]
            }
            "hsla" if channels.len() == 4 => {
                let rgb = hsl_to_rgb(channels[0], channels[1], channels[2])?;
                [rgb[0], rgb[1], rgb[2], parse_alpha(channels[3])?]
            }
            _ => continue,
        };
        return Some((name, close + 1, rgba));
    }
    None
}

fn parse_rgb_channel(raw: &str) -> Option<u8> {
    if let Some(percent) = raw.strip_suffix('%') {
        let value = percent.parse::<f32>().ok()?;
        (0.0..=100.0)
            .contains(&value)
            .then(|| (value * 2.55).round() as u8)
    } else {
        let value = raw.parse::<u8>().ok()?;
        Some(value)
    }
}

fn parse_alpha(raw: &str) -> Option<u8> {
    if let Some(percent) = raw.strip_suffix('%') {
        let value = percent.parse::<f32>().ok()?;
        (0.0..=100.0)
            .contains(&value)
            .then(|| (value * 2.55).round() as u8)
    } else {
        let value = raw.parse::<f32>().ok()?;
        (0.0..=1.0)
            .contains(&value)
            .then(|| (value * 255.0).round() as u8)
    }
}

fn hsl_to_rgb(hue: &str, saturation: &str, lightness: &str) -> Option<[u8; 3]> {
    let hue = hue.trim_end_matches("deg").parse::<f32>().ok()?;
    let saturation = saturation.strip_suffix('%')?.parse::<f32>().ok()? / 100.0;
    let lightness = lightness.strip_suffix('%')?.parse::<f32>().ok()? / 100.0;
    if !(0.0..=100.0).contains(&(saturation * 100.0))
        || !(0.0..=100.0).contains(&(lightness * 100.0))
    {
        return None;
    }
    let hue = hue.rem_euclid(360.0) / 60.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let intermediate = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (red, green, blue) = match hue as u32 {
        0 => (chroma, intermediate, 0.0),
        1 => (intermediate, chroma, 0.0),
        2 => (0.0, chroma, intermediate),
        3 => (0.0, intermediate, chroma),
        4 => (intermediate, 0.0, chroma),
        _ => (chroma, 0.0, intermediate),
    };
    let offset = lightness - chroma / 2.0;
    Some([
        ((red + offset) * 255.0).round() as u8,
        ((green + offset) * 255.0).round() as u8,
        ((blue + offset) * 255.0).round() as u8,
    ])
}

pub fn packed_rgba(rgba: [u8; 4]) -> u32 {
    u32::from_be_bytes(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_short_and_long_hex_colors() {
        let text = "a { color: #fff; bg: #11223380; border: #abcd; }";
        let colors = find_color_literals(text);
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0].rgba, [255, 255, 255, 255]);
        assert_eq!(colors[1].rgba, [0x11, 0x22, 0x33, 0x80]);
        assert_eq!(colors[2].rgba, [0xaa, 0xbb, 0xcc, 0xdd]);
        assert_eq!(&text[colors[1].start..colors[1].end], "#11223380");
    }

    #[test]
    fn rejects_words_and_partial_hex_runs() {
        let text = "issue#fff #1234567 #xyz #12 #abc";
        let colors = find_color_literals(text);
        assert_eq!(colors.len(), 1);
        assert_eq!(&text[colors[0].start..colors[0].end], "#abc");
    }

    #[test]
    fn finds_rgb_rgba_hsl_and_hsla_functions() {
        let text = "rgb(255, 0, 0) rgba(0,0,0,0.5) hsl(120, 100%, 50%) hsla(240, 100%, 50%, 0.25)";
        let colors = find_color_literals(text);
        assert_eq!(colors.len(), 4);
        assert_eq!(colors[0].rgba, [255, 0, 0, 255]);
        assert_eq!(colors[1].rgba, [0, 0, 0, 128]);
        assert_eq!(colors[2].rgba, [0, 255, 0, 255]);
        assert_eq!(colors[3].rgba, [0, 0, 255, 64]);
    }

    #[test]
    fn rejects_out_of_range_and_malformed_color_functions() {
        let text = "rgb(300, 0, 0) rgba(0,0,0,2) hsl(120, 100, 50) hsl(120, 100%, 50%";
        assert!(find_color_literals(text).is_empty());
    }

    #[test]
    fn packs_channels_for_gpui() {
        assert_eq!(packed_rgba([0x11, 0x22, 0x33, 0x44]), 0x11223344);
    }
}
