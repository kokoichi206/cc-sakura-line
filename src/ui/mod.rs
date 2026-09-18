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

use crate::data::{Snapshot, UsageGauge};

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
const GAUGE_LOW: Color = Color::Rgb(154, 199, 122);
const GAUGE_MID: Color = Color::Rgb(226, 168, 92);
const GAUGE_HIGH: Color = Color::Rgb(235, 110, 110);
/// Unused fill. Must stay brighter than the cell background or the gauge length is unreadable.
const GAUGE_EMPTY_FG: Color = Color::Rgb(90, 90, 90);

const LINE_PREFIX: &str = " ";
const COL_PCTS: [u16; 4] = [25, 25, 25, 25];
/// Powerline Extra Symbols semicircles. Write them as code points;
/// PUA literals can drop in edit or transfer.
const ROUND_LEFT: &str = "\u{e0b6}";
const ROUND_RIGHT: &str = "\u{e0b4}";
const PILL_BORDER_WIDTH: usize = 2;

const GAUGE_CELLS: usize = 10;
/// Used and unused cells share one glyph and differ only by color.
/// Dotted glyphs wash out on some fonts, so the full length becomes unreadable.
const GAUGE_CELL: char = '█';
const GAUGE_MID_THRESHOLD: f64 = 50.0;
const GAUGE_HIGH_THRESHOLD: f64 = 80.0;

/// Color lives on the cell, not the column index, so row layout can change without breaking decoration.
#[derive(Clone, Debug)]
enum Cell {
    SakuraPill(String),
    GreenPill(String),
    Block(String),
    GitChanges(String),
    Gauge {
        used_percentage: f64,
        label: String,
    },
    /// Pad so the band's right edge lines up with the grid. Skip drawing when leftover width is 0.
    Spacer,
}

/// Grid rows share four column widths. A longer banner grows only the last column so every row shares one right edge.
#[derive(Clone, Debug)]
enum StatusRow {
    Grid([Cell; 4]),
    Banner(Vec<Cell>),
}

pub fn render(frame: &mut Frame<'_>, snapshot: &Snapshot) {
    let area = frame.size();
    let area = padded_area(area);
    let width = area.width as usize;
    let fill = should_fill();
    let rows = status_rows(snapshot);
    let widths = grid_widths(&rows);

    let lines = rows
        .iter()
        .map(|row| render_line(row, Some(width), fill, widths))
        .collect::<Vec<_>>();

    let total_lines = lines.len().min(area.height as usize);
    let constraints = vec![Constraint::Length(1); total_lines];
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let base_style = Style::default().bg(ROW_BG).fg(ROW_FG);
    for (idx, line) in lines.into_iter().take(total_lines).enumerate() {
        frame.render_widget(Paragraph::new(line).style(base_style), layout[idx]);
    }
}

pub fn format_output(snapshot: &Snapshot) -> String {
    let width = terminal_width();
    let fill = should_fill();
    let rows = status_rows(snapshot);
    let widths = grid_widths(&rows);

    let lines = rows
        .iter()
        .map(|row| format_row(row, width, fill, widths))
        .collect::<Vec<_>>();
    format!("{}\n", lines.join("\n"))
}

fn status_rows(snapshot: &Snapshot) -> Vec<StatusRow> {
    vec![
        StatusRow::Grid([
            Cell::SakuraPill(snapshot.model.clone()),
            Cell::Block(snapshot.version.clone()),
            Cell::GitChanges(snapshot.contributions.clone()),
            Cell::GreenPill(snapshot.session_clock.clone()),
        ]),
        StatusRow::Grid([
            Cell::SakuraPill(snapshot.repository.clone()),
            Cell::Block(snapshot.branch.clone()),
            Cell::GitChanges(snapshot.git_changes.clone()),
            Cell::GreenPill(snapshot.ahead_behind.clone()),
        ]),
        StatusRow::Grid([
            Cell::SakuraPill(snapshot.context.clone()),
            Cell::Block(snapshot.context_remaining.clone()),
            Cell::GitChanges(String::new()),
            Cell::GreenPill(snapshot.now_clock.clone()),
        ]),
        StatusRow::Banner(vec![
            Cell::SakuraPill("5h".to_string()),
            gauge_cell(snapshot.five_hour.as_ref()),
            Cell::Spacer,
            Cell::GreenPill("7d".to_string()),
            gauge_cell(snapshot.seven_day.as_ref()),
        ]),
    ]
}

fn gauge_cell(gauge: Option<&UsageGauge>) -> Cell {
    match gauge {
        Some(gauge) => Cell::Gauge {
            used_percentage: gauge.used_percentage,
            label: format!(
                "{}% used {}",
                gauge.used_percentage.round() as u64,
                gauge.reset_eta
            ),
        },
        None => Cell::Block("-".to_string()),
    }
}

impl Cell {
    fn natural_width(&self) -> usize {
        match self {
            Cell::SakuraPill(text) | Cell::GreenPill(text) => {
                display_width(&padded(text)) + PILL_BORDER_WIDTH
            }
            Cell::Block(text) | Cell::GitChanges(text) => display_width(&padded(text)),
            Cell::Gauge { label, .. } => GAUGE_CELLS + display_width(&padded(label)) + 1,
            Cell::Spacer => 0,
        }
    }
}

/// Size columns from grid rows. If the banner is longer, only the last column grows
/// so short values in columns 0-2 are not stretched.
fn grid_widths(rows: &[StatusRow]) -> [usize; 4] {
    let mut widths = [0usize; 4];
    let mut banner_natural = 0usize;
    for row in rows {
        match row {
            StatusRow::Grid(cells) => {
                for (idx, cell) in cells.iter().enumerate() {
                    widths[idx] = widths[idx].max(cell.natural_width());
                }
            }
            StatusRow::Banner(cells) => {
                banner_natural = banner_natural.max(cells.iter().map(Cell::natural_width).sum());
            }
        }
    }
    let grid_total: usize = widths.iter().sum();
    if banner_natural > grid_total {
        widths[3] += banner_natural - grid_total;
    }
    widths
}

fn cell_widths(
    row: &StatusRow,
    total_width: Option<usize>,
    fill: bool,
    grid: [usize; 4],
) -> Vec<usize> {
    match row {
        StatusRow::Grid(_) => match (fill, total_width) {
            (true, Some(total)) => column_widths(total.saturating_sub(LINE_PREFIX.len())).to_vec(),
            _ => grid.to_vec(),
        },
        StatusRow::Banner(cells) => {
            let mut widths: Vec<usize> = cells.iter().map(Cell::natural_width).collect();
            let target = match (fill, total_width) {
                (true, Some(total)) => total.saturating_sub(LINE_PREFIX.len()),
                _ => grid.iter().sum(),
            };
            let fixed: usize = widths.iter().sum();
            if let Some(idx) = cells.iter().position(|cell| matches!(cell, Cell::Spacer)) {
                widths[idx] = target.saturating_sub(fixed);
            }
            widths
        }
    }
}

fn cells_of(row: &StatusRow) -> &[Cell] {
    match row {
        StatusRow::Grid(cells) => cells.as_slice(),
        StatusRow::Banner(cells) => cells.as_slice(),
    }
}

fn format_row(row: &StatusRow, total_width: Option<usize>, fill: bool, grid: [usize; 4]) -> String {
    let widths = cell_widths(row, total_width, fill, grid);
    let mut out = String::new();
    out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
    out.push_str(LINE_PREFIX);

    let mut remaining = total_width
        .map(|w| w.saturating_sub(LINE_PREFIX.len()))
        .unwrap_or(usize::MAX);
    let mut used = 0usize;

    for (cell, natural) in cells_of(row).iter().zip(widths) {
        let width = if total_width.is_some() {
            natural.min(remaining)
        } else {
            natural
        };
        if width == 0 {
            continue;
        }
        out.push_str(&ansi_cell(cell, width));
        used += width;
        remaining = remaining.saturating_sub(width);
        if total_width.is_some() && remaining == 0 {
            break;
        }
    }

    if fill {
        if let Some(total) = total_width {
            let pad = total.saturating_sub(LINE_PREFIX.len()).saturating_sub(used);
            if pad > 0 {
                out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
                out.extend(std::iter::repeat_n(' ', pad));
            }
        }
    }

    out.push_str("\x1b[0m");
    out
}

fn render_line(
    row: &StatusRow,
    total_width: Option<usize>,
    fill: bool,
    grid: [usize; 4],
) -> Line<'static> {
    let widths = cell_widths(row, total_width, fill, grid);
    let mut spans = Vec::with_capacity(12);
    spans.push(Span::styled(
        LINE_PREFIX,
        Style::default().bg(ROW_BG).fg(ROW_FG),
    ));

    let mut remaining = total_width
        .map(|w| w.saturating_sub(LINE_PREFIX.len()))
        .unwrap_or(usize::MAX);
    let mut used = 0usize;

    for (cell, natural) in cells_of(row).iter().zip(widths) {
        let width = if total_width.is_some() {
            natural.min(remaining)
        } else {
            natural
        };
        if width == 0 {
            continue;
        }
        spans.extend(cell_spans(cell, width));
        used += width;
        remaining = remaining.saturating_sub(width);
        if total_width.is_some() && remaining == 0 {
            break;
        }
    }

    if fill {
        if let Some(total) = total_width {
            let pad = total.saturating_sub(LINE_PREFIX.len()).saturating_sub(used);
            if pad > 0 {
                spans.push(Span::styled(
                    " ".repeat(pad),
                    Style::default().bg(ROW_BG).fg(ROW_FG),
                ));
            }
        }
    }

    Line::from(spans)
}

fn ansi_cell(cell: &Cell, width: usize) -> String {
    match cell {
        Cell::SakuraPill(text) => ansi_pill(text, width, SAKURA, SAKURA_FG),
        Cell::GreenPill(text) => ansi_pill(text, width, GREEN, GREEN_FG),
        Cell::Block(text) => ansi_block(text, width, MID_BG, MID_FG),
        Cell::GitChanges(text) => ansi_git_changes(text, width, MID_BG, MID_FG),
        Cell::Gauge {
            used_percentage,
            label,
        } => ansi_gauge(*used_percentage, label, width),
        Cell::Spacer => ansi_spacer(width),
    }
}

fn ansi_spacer(width: usize) -> String {
    let mut out = String::new();
    out.push_str(&ansi_fg_bg_color(MID_FG, MID_BG));
    out.extend(std::iter::repeat_n(' ', width));
    out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
    out
}

fn cell_spans(cell: &Cell, width: usize) -> Vec<Span<'static>> {
    match cell {
        Cell::SakuraPill(text) => pill_spans(text, width, SAKURA, SAKURA_FG),
        Cell::GreenPill(text) => pill_spans(text, width, GREEN, GREEN_FG),
        Cell::Block(text) => block_spans(text, width, MID_BG, MID_FG),
        Cell::GitChanges(text) => git_changes_spans(text, width, MID_BG, MID_FG),
        Cell::Gauge {
            used_percentage,
            label,
        } => gauge_spans(*used_percentage, label, width),
        Cell::Spacer => vec![Span::styled(
            " ".repeat(width),
            Style::default().fg(MID_FG).bg(MID_BG),
        )],
    }
}

fn gauge_color(used_percentage: f64) -> Color {
    if used_percentage >= GAUGE_HIGH_THRESHOLD {
        GAUGE_HIGH
    } else if used_percentage >= GAUGE_MID_THRESHOLD {
        GAUGE_MID
    } else {
        GAUGE_LOW
    }
}

fn gauge_filled(used_percentage: f64) -> usize {
    ((used_percentage / 100.0) * GAUGE_CELLS as f64).round() as usize
}

fn ansi_gauge(used_percentage: f64, label: &str, width: usize) -> String {
    if width <= GAUGE_CELLS + 1 {
        return ansi_block(label, width, MID_BG, MID_FG);
    }

    let filled = gauge_filled(used_percentage).min(GAUGE_CELLS);
    let mut out = String::new();
    out.push_str(&ansi_fg_bg_color(MID_FG, MID_BG));
    out.push(' ');
    out.push_str(&ansi_fg_bg_color(gauge_color(used_percentage), MID_BG));
    out.extend(std::iter::repeat_n(GAUGE_CELL, filled));
    out.push_str(&ansi_fg_bg_color(GAUGE_EMPTY_FG, MID_BG));
    out.extend(std::iter::repeat_n(GAUGE_CELL, GAUGE_CELLS - filled));
    out.push_str(&ansi_fg_bg_color(MID_FG, MID_BG));
    out.push_str(&fit_cell(&padded(label), width - GAUGE_CELLS - 1));
    out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
    out
}

fn gauge_spans(used_percentage: f64, label: &str, width: usize) -> Vec<Span<'static>> {
    if width <= GAUGE_CELLS + 1 {
        return block_spans(label, width, MID_BG, MID_FG);
    }

    let filled = gauge_filled(used_percentage).min(GAUGE_CELLS);
    vec![
        Span::styled(" ", Style::default().fg(MID_FG).bg(MID_BG)),
        Span::styled(
            GAUGE_CELL.to_string().repeat(filled),
            Style::default().fg(gauge_color(used_percentage)).bg(MID_BG),
        ),
        Span::styled(
            GAUGE_CELL.to_string().repeat(GAUGE_CELLS - filled),
            Style::default().fg(GAUGE_EMPTY_FG).bg(MID_BG),
        ),
        Span::styled(
            fit_cell(&padded(label), width - GAUGE_CELLS - 1),
            Style::default().fg(MID_FG).bg(MID_BG),
        ),
    ]
}

fn pill_spans(value: &str, width: usize, bg: Color, fg: Color) -> Vec<Span<'static>> {
    if width < PILL_BORDER_WIDTH {
        return block_spans(value, width, bg, fg);
    }
    let inner = fit_cell(&padded(value), width - PILL_BORDER_WIDTH);
    vec![
        Span::styled(ROUND_LEFT, Style::default().fg(bg).bg(ROW_BG)),
        Span::styled(inner, Style::default().fg(fg).bg(bg)),
        Span::styled(ROUND_RIGHT, Style::default().fg(bg).bg(ROW_BG)),
    ]
}

fn block_spans(value: &str, width: usize, bg: Color, fg: Color) -> Vec<Span<'static>> {
    vec![Span::styled(
        fit_cell(&padded(value), width),
        Style::default().fg(fg).bg(bg),
    )]
}

fn git_changes_spans(value: &str, width: usize, bg: Color, fg: Color) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }

    let text = fit_cell(&padded(value), width);
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
    let inner = fit_cell(&padded(value), width - PILL_BORDER_WIDTH);
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

    let mut out = String::new();
    out.push_str(&ansi_fg_bg_color(fg, bg));
    out.push_str(&fit_cell(&padded(value), width));
    out.push_str(&ansi_fg_bg(ROW_FG, ROW_BG));
    out
}

fn ansi_git_changes(value: &str, width: usize, bg: Color, fg: Color) -> String {
    if width == 0 {
        return String::new();
    }

    let text = fit_cell(&padded(value), width);
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

fn padded(text: &str) -> String {
    format!(" {} ", text)
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
    let mut widths = [0usize; 4];
    let mut used = 0usize;

    for (idx, pct) in COL_PCTS.iter().enumerate() {
        let w = total_width * (*pct as usize) / 100;
        widths[idx] = w;
        used += w;
    }

    if used < total_width {
        widths[3] += total_width - used;
    }

    widths
}

fn fit_cell(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let mut out = trim_to_width(text, width);
    let pad = width.saturating_sub(display_width(&out));
    out.extend(std::iter::repeat_n(' ', pad));
    out
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
    use super::{
        cell_widths, format_output, format_row, gauge_filled, grid_widths, status_rows, Cell,
        StatusRow, ROUND_LEFT, ROUND_RIGHT,
    };
    use crate::data::{Snapshot, UsageGauge};

    fn filled_snapshot() -> Snapshot {
        Snapshot {
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
            five_hour: Some(UsageGauge {
                used_percentage: 53.0,
                reset_eta: "2h48m".to_string(),
            }),
            seven_day: Some(UsageGauge {
                used_percentage: 9.0,
                reset_eta: "6d5h".to_string(),
            }),
        }
    }

    fn banner_natural_width(row: &StatusRow) -> usize {
        let StatusRow::Banner(cells) = row else {
            panic!("expected a banner row");
        };
        cells.iter().map(Cell::natural_width).sum()
    }

    #[test]
    fn format_output_contains_lines() {
        let output = format_output(&filled_snapshot());
        let lines: Vec<&str> = output.trim_end().split('\n').collect();
        assert!(lines.len() >= 4);
        assert!(output.contains("model"));
        assert!(output.contains("0.1.0"));
        assert!(output.contains("owner/repo"));
        assert!(output.contains("53% used 2h48m"));
        assert!(output.contains("7d"));
        assert!(output.contains("9% used 6d5h"));
    }

    #[test]
    fn rate_limit_row_is_a_banner() {
        let rows = status_rows(&filled_snapshot());
        assert_eq!(rows.len(), 4);

        let StatusRow::Banner(cells) = &rows[3] else {
            panic!("row 4 is a banner");
        };
        assert!(matches!(&cells[0], Cell::SakuraPill(text) if text == "5h"));
        assert!(matches!(&cells[1], Cell::Gauge { label, .. } if label == "53% used 2h48m"));
        assert!(matches!(&cells[2], Cell::Spacer));
        assert!(matches!(&cells[3], Cell::GreenPill(text) if text == "7d"));
        assert!(matches!(&cells[4], Cell::Gauge { label, .. } if label == "9% used 6d5h"));
    }

    #[test]
    fn banner_row_ends_flush_with_the_grid() {
        let mut snapshot = filled_snapshot();
        snapshot.repository = "a-fairly-long-owner/a-fairly-long-repo".to_string();

        let rows = status_rows(&snapshot);
        let grid = grid_widths(&rows);
        let grid_total: usize = grid.iter().sum();
        let banner_total: usize = cell_widths(&rows[3], None, false, grid).iter().sum();

        assert!(
            grid_total > banner_natural_width(&rows[3]),
            "precondition: grid wider than banner"
        );
        assert_eq!(banner_total, grid_total);
    }

    #[test]
    fn banner_row_keeps_content_when_wider_than_the_grid() {
        let rows = status_rows(&filled_snapshot());
        let grid = grid_widths(&rows);
        let natural = banner_natural_width(&rows[3]);
        let grid_total: usize = grid.iter().sum();
        assert_eq!(grid_total, natural, "grid grows to the banner");

        let banner_total: usize = cell_widths(&rows[3], None, false, grid).iter().sum();
        assert_eq!(
            banner_total, natural,
            "spacer is 0; content does not shrink"
        );
    }

    #[test]
    fn all_rows_share_the_longer_right_edge() {
        let short_grid = filled_snapshot();
        let short_rows = status_rows(&short_grid);
        let short_grid_widths = grid_widths(&short_rows);
        assert_eq!(
            short_grid_widths.iter().sum::<usize>(),
            banner_natural_width(&short_rows[3])
        );
        assert_eq!(
            cell_widths(&short_rows[3], None, false, short_grid_widths)
                .iter()
                .sum::<usize>(),
            short_grid_widths.iter().sum::<usize>()
        );

        let mut long_grid = filled_snapshot();
        long_grid.repository = "a-fairly-long-owner/a-fairly-long-repo".to_string();
        let long_rows = status_rows(&long_grid);
        let long_grid_widths = grid_widths(&long_rows);
        let grid_total: usize = long_grid_widths.iter().sum();
        assert!(grid_total > banner_natural_width(&long_rows[3]));
        assert_eq!(
            cell_widths(&long_rows[3], None, false, long_grid_widths)
                .iter()
                .sum::<usize>(),
            grid_total
        );
    }

    #[test]
    fn banner_keeps_seven_day_when_spacer_is_zero() {
        let rows = status_rows(&filled_snapshot());
        let grid = grid_widths(&rows);

        let spacer = cell_widths(&rows[3], Some(80), false, grid)[2];
        assert_eq!(spacer, 0);

        let line = format_row(&rows[3], Some(80), false, grid);
        assert!(line.contains("7d"), "zero-width spacer must not drop 7d");
        assert!(line.contains("9% used 6d5h"));
    }

    #[test]
    fn longer_banner_stretches_only_the_last_grid_column() {
        let mut snapshot = filled_snapshot();
        snapshot.branch = "short".to_string();
        snapshot.five_hour = None;
        snapshot.seven_day = None;
        let without_gauges = grid_widths(&status_rows(&snapshot));

        snapshot.five_hour = Some(UsageGauge {
            used_percentage: 53.0,
            reset_eta: "a-very-long-reset-label-must-not-move-columns".to_string(),
        });
        snapshot.seven_day = Some(UsageGauge {
            used_percentage: 9.0,
            reset_eta: "6d5h".to_string(),
        });
        let with_gauges = grid_widths(&status_rows(&snapshot));

        assert_eq!(without_gauges[0], with_gauges[0]);
        assert_eq!(without_gauges[1], with_gauges[1]);
        assert_eq!(without_gauges[2], with_gauges[2]);
        assert!(with_gauges[3] > without_gauges[3]);
        assert_eq!(
            with_gauges.iter().sum::<usize>(),
            banner_natural_width(&status_rows(&snapshot)[3])
        );
    }

    #[test]
    fn missing_rate_limits_fall_back_to_placeholder() {
        let mut snapshot = filled_snapshot();
        snapshot.five_hour = None;
        snapshot.seven_day = None;

        let rows = status_rows(&snapshot);
        let StatusRow::Banner(cells) = &rows[3] else {
            panic!("row 4 is a banner");
        };
        assert!(matches!(&cells[1], Cell::Block(text) if text == "-"));
        assert!(matches!(&cells[4], Cell::Block(text) if text == "-"));
    }

    #[test]
    fn pills_keep_their_rounded_ends() {
        assert_eq!(ROUND_LEFT.chars().count(), 1);
        assert_eq!(ROUND_RIGHT.chars().count(), 1);

        let output = format_output(&filled_snapshot());
        assert!(output.contains(ROUND_LEFT), "left rounded cap is drawn");
        assert!(output.contains(ROUND_RIGHT), "right rounded cap is drawn");
    }

    #[test]
    fn gauge_filled_rounds_to_cells() {
        assert_eq!(gauge_filled(0.0), 0);
        assert_eq!(gauge_filled(4.9), 0);
        assert_eq!(gauge_filled(5.0), 1);
        assert_eq!(gauge_filled(53.0), 5);
        assert_eq!(gauge_filled(100.0), 10);
    }
}
