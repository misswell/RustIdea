#[path = "../src/terminal_display.rs"]
mod terminal_display;

use terminal_display::terminal_display_lines;

#[test]
fn ansi_status_sequences_and_carriage_return_do_not_corrupt_output() {
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
fn terminal_erases_and_tabs_without_leaking_control_bytes() {
    assert_eq!(terminal_display_lines("abc\u{8}d"), ["abd"]);
    assert_eq!(terminal_display_lines("old\x1b[2Knew"), ["new"]);
    assert_eq!(terminal_display_lines("a\tb"), ["a    b"]);
    assert!(
        !terminal_display_lines("\x1b[31mred\x1b[0m")
            .iter()
            .any(|line| line.contains('\x1b'))
    );
}

#[test]
fn terminal_output_is_bounded() {
    let output = (0..5_010)
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    let lines = terminal_display_lines(&output);
    assert_eq!(lines.len(), 5_000);
    assert_eq!(lines.first().map(String::as_str), Some("11"));
}
