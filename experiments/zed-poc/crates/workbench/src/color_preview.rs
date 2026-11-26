#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorLiteral {
    pub start: usize,
    pub end: usize,
    pub rgba: [u8; 4],
}

pub fn find_color_literals(text: &str) -> Vec<ColorLiteral> {
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
    fn packs_channels_for_gpui() {
        assert_eq!(packed_rgba([0x11, 0x22, 0x33, 0x44]), 0x11223344);
    }
}
