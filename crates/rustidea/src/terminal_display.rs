/// Minimal terminal screen model for shell output. Escape sequences are
/// consumed instead of printed; carriage returns, backspaces and the common
/// erase-line/screen commands are applied. Full VT colour is intentionally
/// not faked with literal escape text.
pub fn terminal_display_lines(output: &str) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut chars = output.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\x1b' => match chars.next() {
                Some('[') => {
                    let mut last = '\0';
                    for code in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&code) {
                            last = code;
                            break;
                        }
                    }
                    match last {
                        'K' => {
                            if let Some(line) = lines.last_mut() {
                                line.clear();
                            }
                        }
                        'J' => {
                            lines.clear();
                            lines.push(String::new());
                        }
                        _ => {}
                    }
                }
                Some(']') => {
                    let mut previous = '\0';
                    for code in chars.by_ref() {
                        if code == '\x07' || (previous == '\x1b' && code == '\\') {
                            break;
                        }
                        previous = code;
                    }
                }
                Some(_) | None => {}
            },
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    lines.push(String::new());
                } else if let Some(line) = lines.last_mut() {
                    line.clear();
                }
            }
            '\n' => lines.push(String::new()),
            '\u{8}' => {
                if let Some(line) = lines.last_mut() {
                    line.pop();
                }
            }
            '\t' => {
                if let Some(line) = lines.last_mut() {
                    line.push_str("    ");
                }
            }
            '\x07' | '\x00'..='\x08' | '\x0b'..='\x1f' => {}
            character => {
                if let Some(line) = lines.last_mut() {
                    line.push(character);
                }
            }
        }
    }
    if lines.len() > 5000 {
        lines.drain(..lines.len() - 5000);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::terminal_display_lines;

    #[test]
    fn terminal_consumes_ansi_and_applies_carriage_returns() {
        assert_eq!(
            terminal_display_lines("\x1b]0;title\x07\x1b[31mred\x1b[0m\rnext\n"),
            ["next", ""]
        );
        assert_eq!(
            terminal_display_lines("progress 10%\rprogress 20%"),
            ["progress 20%"]
        );
    }

    #[test]
    fn terminal_applies_backspace_erase_line_and_tabs() {
        assert_eq!(terminal_display_lines("abc\u{8}d"), ["abd"]);
        assert_eq!(terminal_display_lines("old\x1b[2Knew"), ["new"]);
        assert_eq!(terminal_display_lines("a\tb"), ["a    b"]);
    }

    #[test]
    fn terminal_output_keeps_the_last_five_thousand_lines() {
        let output = (0..5_010)
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        let lines = terminal_display_lines(&output);
        assert_eq!(lines.len(), 5_000);
        assert_eq!(lines.first().map(String::as_str), Some("11"));
    }
}
