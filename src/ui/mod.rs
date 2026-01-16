use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::{
    fs::File,
    process::{Command, Stdio},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::data::Snapshot;

const SAKURA: Color = Color::Rgb(241, 157, 181);
const SAKURA_FG: Color = Color::Rgb(35, 30, 30);
const GREEN: Color = Color::Rgb(154, 199, 122);
const GREEN_FG: Color = Color::Rgb(30, 45, 28);
const ROW_BG: Color = Color::Rgb(40, 40, 40);
const ROW_FG: Color = Color::Rgb(220, 220, 220);
const MID_BG: Color = Color::Rgb(55, 55, 55);
const MID_FG: Color = Color::Rgb(220, 220, 220);
const PLUS_FG: Color = Color::Rgb(98, 201, 98);
const MINUS_FG: Color = Color::Rgb(235, 110, 110);
const LINE_PREFIX: &str = " ";
const COL_PCTS: [u16; 4] = [25, 25, 25, 25];
const ROUND_LEFT: &str = "";
const ROUND_RIGHT: &str = "";
const PILL_BORDER_WIDTH: usize = 2;

type Segment<'a> = (usize, &'a str);

pub fn render(frame: &mut Frame<'_>, snapshot: &Snapshot) {
    let area = frame.size();
    let area = padded_area(area);
    let width = area.width as usize;
    let fill = should_fill();
    let shared = if fill {
        None
    } else {
        Some(shared_widths(snapshot))
    };

    let lines = build_lines(snapshot, fill)
        .into_iter()
        .map(|segments| render_line(Some(width), &segments, fill, shared))
        .collect::<Vec<_>>();

    let total_lines = lines.len().min(area.height as usize);
    let constraints = vec![Constraint::Length(1); total_lines];
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let base_style = Style::default().bg(ROW_BG).fg(ROW_FG);
    for (idx, line) in lines.into_iter().take(total_lines).enumerate() {
        frame.render_widget(Paragraph::new(line).style(base_style), rows[idx]);
    }
}

pub fn format_output(snapshot: &Snapshot) -> String {
    let width = terminal_width();
    let fill = should_fill();
    let shared = if fill {
        None
    } else {
        Some(shared_widths(snapshot))
    };
    let lines = build_lines(snapshot, fill)
        .into_iter()
        .map(|segments| format_row(&segments, width, fill, shared))
        .collect::<Vec<_>>();
    format!("{}\n", lines.join("\n"))
}

fn build_lines<'a>(snapshot: &'a Snapshot, fill: bool) -> Vec<Vec<Segment<'a>>> {
    // Row 1: Claude info
    let row1 = [
        snapshot.model.as_str(),
        snapshot.version.as_str(),
        snapshot.contributions.as_str(),
        snapshot.session_clock.as_str(),
    ];
    // Row 2: Git info
    let row2 = [
        snapshot.repository.as_str(),
        snapshot.branch.as_str(),
        snapshot.git_changes.as_str(),
        snapshot.ahead_behind.as_str(),
    ];
    // Row 3: Context info
    let row3 = [
        snapshot.context.as_str(),
        snapshot.context_remaining.as_str(),
        "",
        snapshot.now_clock.as_str(),
    ];

    let mut lines = Vec::new();
    lines.extend(split_segments(row1, fill));
    lines.extend(split_segments(row2, fill));
    lines.extend(split_segments(row3, fill));
    lines
}

fn split_segments<'a>(row: [&'a str; 4], fill: bool) -> Vec<Vec<Segment<'a>>> {
    let segments: Vec<Segment<'a>> = row
        .iter()
        .enumerate()
        .map(|(idx, value)| (idx, *value))
        .collect();

    if fill {
        return vec![segments];
    }

    vec![segments]
}

fn format_row(
    segments: &[Segment<'_>],
    width_opt: Option<usize>,
    fill: bool,
    shared_widths: Option<[usize; 4]>,
) -> String {
    let widths = if fill {
        width_opt.map(|w| column_widths(w.saturating_sub(LINE_PREFIX.len())))
    } else {
        shared_widths
    };
    let mut out = String::new();
    let row_style = ansi_fg_bg(ROW_FG, ROW_BG);
    out.push_str(&row_style);
    out.push_str(LINE_PREFIX);

    let mut remaining = width_opt
        .map(|w| w.saturating_sub(LINE_PREFIX.len()))
        .unwrap_or(usize::MAX);

    for (idx, value) in segments.iter() {
        let width = match widths {
            Some(cols) => {
                let mut w = cols[*idx];
                if width_opt.is_some() {
                    w = w.min(remaining);
                }
                w
            }
            None => {
                let natural = natural_width(value, *idx);
                if width_opt.is_some() {
                    natural.min(remaining)
                } else {
                    natural
                }
            }
        };
        if width == 0 {
            break;
        }
        let segment = match *idx {
            0 => ansi_pill(value, width, SAKURA, SAKURA_FG),
            3 => ansi_pill(value, width, GREEN, GREEN_FG),
            2 => ansi_git_changes(value, width, MID_BG, MID_FG),
            _ => ansi_block(value, width, MID_BG, MID_FG),
        };
        out.push_str(&segment);

        if width_opt.is_some() {
            remaining = remaining.saturating_sub(width);
            if remaining == 0 {
                break;
            }
        }
    }

    out.push_str("\x1b[0m");
    out
}

fn padded_area(area: Rect) -> Rect {
    if area.width <= 1 {
        return area;
    }
    Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width - 1,
        height: area.height,
    }
}

fn column_widths(total_width: usize) -> [usize; 4] {
    let usable = total_width;
    let mut widths = [0usize; 4];
    let mut used = 0usize;

    for (idx, pct) in COL_PCTS.iter().enumerate() {
        let w = usable * (*pct as usize) / 100;
        widths[idx] = w;
        used += w;
    }

    if used < usable {
        widths[3] += usable - used;
    }

    widths
}

fn fit_cell(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let trimmed = trim_to_width(text, width);
    let mut out = trimmed;
    let pad = width.saturating_sub(display_width(&out));
    out.extend(std::iter::repeat_n(' ', pad));
    out
}

fn segment_text(value: &str) -> String {
    format!(" {} ", value)
}

fn render_line(
    width_opt: Option<usize>,
    segments: &[Segment<'_>],
    fill: bool,
    shared_widths: Option<[usize; 4]>,
) -> Line<'static> {
    let widths = if fill {
        width_opt.map(|w| column_widths(w.saturating_sub(LINE_PREFIX.len())))
    } else {
        shared_widths
    };
    let mut spans = Vec::with_capacity(10);

    spans.push(Span::styled(
        LINE_PREFIX,
        Style::default().bg(ROW_BG).fg(ROW_FG),
    ));

    let mut remaining = width_opt
        .map(|w| w.saturating_sub(LINE_PREFIX.len()))
        .unwrap_or(usize::MAX);

    for (idx, value) in segments.iter() {
        let width = match widths {
            Some(cols) => {
                let mut w = cols[*idx];
                if width_opt.is_some() {
                    w = w.min(remaining);
                }
                w
            }
            None => {
                let natural = natural_width(value, *idx);
                if width_opt.is_some() {
                    natural.min(remaining)
                } else {
                    natural
                }
            }
        };
        if width == 0 {
            break;
        }

        let (bg, fg) = match *idx {
            0 => (SAKURA, SAKURA_FG),
            3 => (GREEN, GREEN_FG),
            _ => (MID_BG, MID_FG),
        };

        let segment = if *idx == 0 || *idx == 3 {
            pill_spans(value, width, bg, fg)
        } else if *idx == 2 {
            git_changes_spans(value, width, bg, fg)
        } else {
            block_spans(value, width, bg, fg)
        };
        spans.extend(segment);

        if width_opt.is_some() {
            remaining = remaining.saturating_sub(width);
            if remaining == 0 {
                break;
            }
        }
    }

    Line::from(spans)
}

fn shared_widths(snapshot: &Snapshot) -> [usize; 4] {
    let row1 = [
        snapshot.model.as_str(),
        snapshot.version.as_str(),
        snapshot.contributions.as_str(),
        snapshot.session_clock.as_str(),
    ];
    let row2 = [
        snapshot.repository.as_str(),
        snapshot.branch.as_str(),
        snapshot.git_changes.as_str(),
        snapshot.ahead_behind.as_str(),
    ];
    let row3 = [
        snapshot.context.as_str(),
        snapshot.context_remaining.as_str(),
        "",
        snapshot.now_clock.as_str(),
    ];

    let mut widths = [0usize; 4];
    for (idx, value) in row1.iter().enumerate() {
        widths[idx] = widths[idx].max(natural_width(value, idx));
    }
    for (idx, value) in row2.iter().enumerate() {
        widths[idx] = widths[idx].max(natural_width(value, idx));
    }
    for (idx, value) in row3.iter().enumerate() {
        widths[idx] = widths[idx].max(natural_width(value, idx));
    }
    widths
}

fn pill_spans(value: &str, width: usize, bg: Color, fg: Color) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if width < PILL_BORDER_WIDTH {
        return block_spans(value, width, bg, fg);
    }
    let inner_width = width.saturating_sub(PILL_BORDER_WIDTH);
    let inner = fit_cell(&segment_text(value), inner_width);

    spans.push(Span::styled(ROUND_LEFT, Style::default().fg(bg).bg(ROW_BG)));
    spans.push(Span::styled(inner, Style::default().fg(fg).bg(bg)));
    spans.push(Span::styled(
        ROUND_RIGHT,
        Style::default().fg(bg).bg(ROW_BG),
    ));

    spans
}

fn block_spans(value: &str, width: usize, bg: Color, fg: Color) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let text = fit_cell(&segment_text(value), width);
    spans.push(Span::styled(text, Style::default().fg(fg).bg(bg)));
    spans
}

fn git_changes_spans(value: &str, width: usize, bg: Color, fg: Color) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }

    let text = fit_cell(&segment_text(value), width);
    let mut spans = Vec::new();
    let mut buffer = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if (ch == '+' || ch == '-') && chars.peek().is_some_and(|c| c.is_ascii_digit()) {
            if !buffer.is_empty() {
                spans.push(Span::styled(buffer.clone(), Style::default().fg(fg).bg(bg)));
                buffer.clear();
            }

            let mut token = String::new();
            token.push(ch);
            while let Some(next) = chars.peek() {
                if next.is_ascii_digit() {
                    token.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }

            let color = if ch == '+' { PLUS_FG } else { MINUS_FG };
            spans.push(Span::styled(token, Style::default().fg(color).bg(bg)));
        } else {
            buffer.push(ch);
        }
    }

    if !buffer.is_empty() {
        spans.push(Span::styled(buffer, Style::default().fg(fg).bg(bg)));
    }

    spans
}

fn ansi_pill(value: &str, width: usize, bg: Color, fg: Color) -> String {
    if width < PILL_BORDER_WIDTH {
        return ansi_block(value, width, bg, fg);
    }
    let inner_width = width.saturating_sub(PILL_BORDER_WIDTH);
    let inner = fit_cell(&segment_text(value), inner_width);
    let mut out = String::new();

    out.push_str(&ansi_fg_bg_color(bg, ROW_BG));
    out.push_str(ROUND_LEFT);
    out.push_str(&ansi_fg_bg_color(fg, bg));
    out.push_str(&inner);
    out.push_str(&ansi_fg_bg_color(bg, ROW_BG));
    out.push_str(ROUND_RIGHT);
    out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
    out
}

fn ansi_block(value: &str, width: usize, bg: Color, fg: Color) -> String {
    if width == 0 {
        return String::new();
    }

    let text = fit_cell(&segment_text(value), width);
    let mut out = String::new();
    out.push_str(&ansi_fg_bg_color(fg, bg));
    out.push_str(&text);
    out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
    out
}

fn ansi_git_changes(value: &str, width: usize, bg: Color, fg: Color) -> String {
    if width == 0 {
        return String::new();
    }

    let text = fit_cell(&segment_text(value), width);
    let mut out = String::new();
    out.push_str(&ansi_fg_bg_color(fg, bg));

    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if (ch == '+' || ch == '-') && chars.peek().is_some_and(|c| c.is_ascii_digit()) {
            let mut token = String::new();
            token.push(ch);
            while let Some(next) = chars.peek() {
                if next.is_ascii_digit() {
                    token.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }

            let color = if ch == '+' { PLUS_FG } else { MINUS_FG };
            out.push_str(&ansi_fg_bg_color(color, bg));
            out.push_str(&token);
            out.push_str(&ansi_fg_bg_color(fg, bg));
        } else {
            out.push(ch);
        }
    }

    out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
    out
}

fn natural_width(value: &str, idx: usize) -> usize {
    let base = display_width(&segment_text(value));
    if idx == 0 || idx == 3 {
        base + 2
    } else {
        base
    }
}

fn ansi_fg_bg(fg: Color, bg: Color) -> String {
    ansi_fg_bg_color(fg, bg)
}

fn ansi_fg_bg_color(fg: Color, bg: Color) -> String {
    let (fr, fg_c, fb) = rgb(fg);
    let (br, bg_c, bb) = rgb(bg);
    format!(
        "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m",
        fr, fg_c, fb, br, bg_c, bb
    )
}

fn rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 255, 255),
    }
}

fn should_fill() -> bool {
    match std::env::var("CC_STATUSLINE_FILL") {
        Ok(val) => matches!(val.as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}

fn terminal_width() -> Option<usize> {
    if let Ok(val) = std::env::var("CC_STATUSLINE_WIDTH") {
        if let Ok(width) = val.parse::<usize>() {
            return Some(apply_width_adjustments(width));
        }
    }

    if let Ok(val) = std::env::var("COLUMNS") {
        if let Ok(width) = val.parse::<usize>() {
            return Some(apply_width_adjustments(width));
        }
    }

    tput_cols().map(apply_width_adjustments)
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn trim_to_width(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > width {
            break;
        }
        used += ch_width;
        out.push(ch);
    }
    out
}

fn tput_cols() -> Option<usize> {
    let tty = File::open("/dev/tty").ok();

    if let Some(cols) = run_cols_cmd("tput", &["cols"], tty.as_ref()) {
        return Some(cols);
    }

    if let Some(cols) = stty_cols(tty.as_ref()) {
        return Some(cols);
    }

    None
}

fn apply_width_adjustments(width: usize) -> usize {
    let reserved = std::env::var("CC_STATUSLINE_RESERVED")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    width.saturating_sub(reserved)
}

fn run_cols_cmd(cmd: &str, args: &[&str], tty: Option<&File>) -> Option<usize> {
    let mut command = Command::new(cmd);
    command.args(args);
    if let Some(tty) = tty {
        command.stdin(Stdio::from(tty.try_clone().ok()?));
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<usize>()
        .ok()
}

fn stty_cols(tty: Option<&File>) -> Option<usize> {
    let mut command = Command::new("stty");
    command.arg("size");
    if let Some(tty) = tty {
        command.stdin(Stdio::from(tty.try_clone().ok()?));
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let _rows = parts.next();
    let cols = parts.next()?;
    cols.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::format_output;
    use crate::data::Snapshot;

    #[test]
    fn format_output_contains_lines() {
        let snapshot = Snapshot {
            model: "model".to_string(),
            version: "0.1.0".to_string(),
            contributions: "🌲 9".to_string(),
            session_clock: "5h32m".to_string(),
            repository: "owner/repo".to_string(),
            branch: "main".to_string(),
            git_changes: "+3 -1".to_string(),
            ahead_behind: "↑1 ↓0".to_string(),
            context: "10K/100K".to_string(),
            context_remaining: "90% left".to_string(),
            now_clock: "12:34:56".to_string(),
        };

        let output = format_output(&snapshot);
        let lines: Vec<&str> = output.trim_end().split('\n').collect();
        assert!(lines.len() >= 3);
        assert!(output.contains("model"));
        assert!(output.contains("0.1.0"));
        assert!(output.contains("owner/repo"));
    }
}
