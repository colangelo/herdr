//! Draws the display panes labels over each pane and the size summary in the
//! mode bar. State and label contents live in [`crate::app::display_panes`].

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::menus::render_bottom_bar;
use super::text::{display_width, display_width_u16, truncate_end};
use super::widgets::{panel_contrast_fg, render_panel_shell};
use crate::app::{display_panes::DisplayPaneLabel, state::AppState};

/// A label is a bordered panel: one border column and one padding column on
/// each side of its text.
const LABEL_CHROME_COLS: u16 = 4;
const LABEL_ROWS: u16 = 3;
/// Narrower than this and the name is dropped rather than cut to a stub.
const MIN_NAME_COLS: usize = 4;

pub(super) fn render_display_panes(app: &AppState, frame: &mut Frame, mode_bar_area: Rect) {
    let labels = app.display_panes_labels();
    for label in &labels {
        render_label(app, frame, label, true);
    }
    render_summary_bar(app, frame, mode_bar_area, &labels);
    render_sidebar_section_sizes(app, frame);
}

/// The same labels while a pane is resized (fork issue 122), without the
/// digit (nothing to press) and without the summary bar: the mode that is on
/// keeps its own bar.
pub(super) fn render_resize_labels(app: &AppState, frame: &mut Frame) {
    for label in &app.display_panes_labels() {
        render_label(app, frame, label, false);
    }
}

/// The chip style of the labels mode: the theme's red, reversed, as the
/// close-workspace confirmation draws its action.
fn mode_chip_style(app: &AppState) -> Style {
    Style::default()
        .fg(panel_contrast_fg(&app.palette))
        .bg(app.palette.red)
        .add_modifier(Modifier::BOLD)
}

/// Each sidebar section's size (`26x14`), right-aligned on its bottom row.
/// A collapsed sidebar, or a section with no room, shows none.
fn render_sidebar_section_sizes(app: &AppState, frame: &mut Frame) {
    let sidebar = app.view.sidebar_rect;
    if app.sidebar_collapsed || sidebar.width <= 1 || sidebar.height == 0 {
        return;
    }
    let (spaces, agents) = super::expanded_sidebar_sections(sidebar, app.sidebar_section_split);
    for section in [spaces, agents] {
        if section.width == 0 || section.height == 0 {
            continue;
        }
        let text = format!(" {}x{} ", section.width, section.height);
        let width = display_width_u16(&text);
        if width > section.width {
            continue;
        }
        let area = Rect::new(
            section.x + section.width - width,
            section.y + section.height - 1,
            width,
            1,
        );
        frame.render_widget(Paragraph::new(text).style(mode_chip_style(app)), area);
    }
}

/// `2  w5:p16 · claude  138x27`, cut to `max_width`: the name goes first, then
/// the tail of whatever is left.
fn label_text(label: &DisplayPaneLabel, max_width: usize, numbered: bool) -> String {
    let head = if numbered {
        let index = label
            .index
            .map_or_else(|| "·".to_string(), |index| index.to_string());
        format!("{index}  {}", label.address)
    } else {
        label.address.clone()
    };
    let size = format!("{}x{}", label.inner_rect.width, label.inner_rect.height);
    let full = format!("{head} · {}  {size}", label.name);
    if display_width(&full) <= max_width {
        return full;
    }
    let without_name = display_width(&head) + display_width(" · ") + 2 + display_width(&size);
    let name_budget = max_width.saturating_sub(without_name);
    if name_budget >= MIN_NAME_COLS {
        return format!(
            "{head} · {}  {size}",
            truncate_end(&label.name, name_budget)
        );
    }
    truncate_end(&format!("{head}  {size}"), max_width)
}

fn render_label(app: &AppState, frame: &mut Frame, label: &DisplayPaneLabel, numbered: bool) {
    let p = &app.palette;
    let pane = label.inner_rect.intersection(frame.area());
    if pane.is_empty() {
        return;
    }
    let boxed = pane.height >= LABEL_ROWS && pane.width > LABEL_CHROME_COLS;
    let max_text = if boxed {
        pane.width - LABEL_CHROME_COLS
    } else {
        pane.width
    };
    let text = label_text(label, usize::from(max_text), numbered);
    let text_width = display_width_u16(&text);

    let border = app.pane_border_color(label.focused);
    let text_style = Style::default()
        .fg(p.text)
        .bg(p.panel_bg)
        .add_modifier(Modifier::BOLD);
    let line = Line::from(Span::styled(text, text_style));

    if boxed {
        let width = text_width + LABEL_CHROME_COLS;
        let area = Rect::new(
            pane.x + (pane.width - width) / 2,
            pane.y + (pane.height - LABEL_ROWS) / 2,
            width,
            LABEL_ROWS,
        );
        if let Some(inner) = render_panel_shell(frame, area, border, p.panel_bg) {
            let text_area = Rect::new(inner.x + 1, inner.y, text_width, 1);
            frame.render_widget(Paragraph::new(line), text_area);
        }
    } else {
        let area = Rect::new(pane.x, pane.y + pane.height / 2, text_width, 1);
        frame.render_widget(Paragraph::new(line), area);
    }
}

/// ` PANES  window 310x56 · panes 281x55  1-3 focus  any key close  VERSION  0.8.2-…`: the
/// window is the whole frame, the panes figure the area the tab's panes
/// share.
fn render_summary_bar(app: &AppState, frame: &mut Frame, area: Rect, labels: &[DisplayPaneLabel]) {
    let p = &app.palette;
    let key = Style::default().fg(p.red).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(p.overlay0);
    let value = Style::default().fg(p.text);
    let mode_style = mode_chip_style(app);
    let window = frame.area();
    let panes = app.view.terminal_area;

    let mut spans = vec![
        Span::styled(" PANES ", mode_style),
        Span::raw("  "),
        Span::styled("window ", dim),
        Span::styled(format!("{}x{}", window.width, window.height), value),
        Span::styled(" · panes ", dim),
        Span::styled(format!("{}x{}", panes.width, panes.height), value),
        Span::raw("  "),
    ];
    let numbered = labels.iter().filter_map(|label| label.index).max();
    if let Some(last) = numbered {
        let range = if last == 1 {
            "1".to_string()
        } else {
            format!("1-{last}")
        };
        spans.push(Span::styled(range, key));
        spans.push(Span::styled(" focus  ", dim));
    }
    spans.push(Span::styled("any key", key));
    spans.push(Span::styled(" close", dim));

    let bar = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(1),
        area.width,
        1,
    );

    // The build this session runs. The server draws this bar, so after a
    // live handoff it names the server's build even if an older client is
    // still attached. It goes first when the bar is too narrow, so the key
    // hints always survive.
    let version = crate::build_info::version();
    let version_spans = [
        Span::raw("  "),
        Span::styled(" VERSION ", mode_style),
        Span::raw(" "),
        Span::styled(version, value),
    ];
    let used: usize = spans.iter().map(|span| span.width()).sum();
    let wanted: usize = version_spans.iter().map(|span| span.width()).sum();
    if used + wanted <= usize::from(bar.width) {
        spans.extend(version_spans);
    }
    render_bottom_bar(frame, bar, Line::from(spans), p.panel_bg);
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::layout::{Direction, Rect};

    use crate::app::state::{AppState, Mode};
    use crate::ui::test_support::{draw_sized, layout_sized, overlay_snapshot_of, rect_rows};

    /// Wide enough that two side-by-side labels fit uncut; the fixed
    /// snapshot size below covers the placement.
    const WIDTH: u16 = 140;
    const HEIGHT: u16 = 30;

    fn two_pane_app() -> AppState {
        let mut app = AppState::test_new();
        app.workspaces = vec![crate::workspace::Workspace::test_new("one")];
        app.workspaces[0].test_split(Direction::Horizontal);
        app.active = Some(0);
        app.selected = 0;
        app.mode = Mode::Terminal;
        app.ensure_test_terminals();
        layout_sized(&mut app, WIDTH, HEIGHT);
        app
    }

    fn screen(app: &AppState) -> Vec<String> {
        let buffer = draw_sized(app, WIDTH, HEIGHT);
        rect_rows(&buffer, Rect::new(0, 0, WIDTH, HEIGHT))
    }

    #[test]
    fn every_pane_carries_its_number_address_name_and_size() {
        let mut app = two_pane_app();
        app.open_display_panes(Instant::now());
        layout_sized(&mut app, WIDTH, HEIGHT);

        let labels = app.display_panes_labels();
        assert_eq!(labels.len(), 2);
        let rows = screen(&app);
        for label in &labels {
            let text = format!(
                "{}  {} · {}  {}x{}",
                label.index.expect("numbered"),
                label.address,
                label.name,
                label.inner_rect.width,
                label.inner_rect.height
            );
            let row = rows
                .iter()
                .position(|row| row.contains(&text))
                .unwrap_or_else(|| panic!("{text:?} is on screen:\n{}", rows.join("\n")));
            let inner = label.inner_rect;
            assert!(
                (inner.y..inner.y + inner.height).contains(&(row as u16)),
                "{text:?} is drawn inside its own pane"
            );
        }
    }

    /// Fork issue 122: while resizing, each pane shows its label without the
    /// digit, over the resize mode, whose bar stays.
    #[test]
    fn resize_mode_labels_every_pane_and_keeps_its_bar() {
        let mut app = two_pane_app();
        app.mode = Mode::Resize;
        layout_sized(&mut app, WIDTH, HEIGHT);

        let rows = screen(&app);
        for label in app.display_panes_labels() {
            let text = format!(
                "{} · {}  {}x{}",
                label.address, label.name, label.inner_rect.width, label.inner_rect.height
            );
            let row = rows
                .iter()
                .find(|row| row.contains(&text))
                .unwrap_or_else(|| panic!("{text:?} is on screen:\n{}", rows.join("\n")));
            let index = label.index.expect("numbered");
            assert!(
                !row.contains(&format!("{index}  {}", label.address)),
                "no digit to press while resizing: {row:?}"
            );
        }
        let bar = rows.last().unwrap();
        assert!(bar.contains("RESIZE"), "{bar:?}");
        assert!(!bar.contains("PANES"), "{bar:?}");
    }

    #[test]
    fn no_labels_when_nothing_is_resized() {
        let app = two_pane_app();
        let rows = screen(&app);
        let label = &app.display_panes_labels()[0];
        let size = format!("{}x{}", label.inner_rect.width, label.inner_rect.height);
        assert!(!rows
            .iter()
            .any(|row| row.contains(&format!("{}  {size}", label.name))));
    }

    #[test]
    fn the_mode_bar_carries_the_window_and_pane_area_sizes() {
        let mut app = two_pane_app();
        app.open_display_panes(Instant::now());
        layout_sized(&mut app, WIDTH, HEIGHT);

        let area = app.view.terminal_area;
        let text = format!(
            "window {WIDTH}x{HEIGHT} · panes {}x{}",
            area.width, area.height
        );
        let rows = screen(&app);
        assert!(
            rows.iter().any(|row| row.contains(&text)),
            "{text:?} is on screen:\n{}",
            rows.join("\n")
        );
    }

    fn mode_bar_row(app: &AppState, width: u16, height: u16) -> (String, ratatui::buffer::Buffer) {
        let buffer = draw_sized(app, width, height);
        let row =
            crate::ui::test_support::row_text_trimmed(&buffer, Rect::new(0, height - 1, width, 1));
        (row, buffer)
    }

    /// #117: after `any key close` the bar names the build it runs: a
    /// `VERSION` chip styled like `PANES`, then the full version string. The
    /// server draws this bar, so it is the server's version even when a live
    /// handoff left an older client attached.
    #[test]
    fn the_mode_bar_ends_with_the_version_chip() {
        let mut app = two_pane_app();
        app.open_display_panes(Instant::now());
        layout_sized(&mut app, WIDTH, HEIGHT);

        let (row, buffer) = mode_bar_row(&app, WIDTH, HEIGHT);
        let version = crate::build_info::version();
        assert!(
            row.ends_with(&format!("any key close   VERSION  {version}")),
            "{row:?}"
        );

        let col = |text: &str| row[..row.find(text).expect("on the bar")].chars().count() as u16;
        let chip = buffer[(col("VERSION"), HEIGHT - 1)].style();
        let panes = buffer[(col("PANES"), HEIGHT - 1)].style();
        assert_eq!(chip, panes, "the VERSION chip is styled like PANES");
        assert_eq!(
            buffer[(col(&version), HEIGHT - 1)].style().fg,
            Some(app.palette.text),
            "the version is in the normal colour"
        );
    }

    /// A bar too narrow for everything drops the version first and keeps the
    /// key hints.
    #[test]
    fn a_narrow_mode_bar_drops_the_version_before_the_key_hints() {
        let width = 90;
        let mut app = two_pane_app();
        layout_sized(&mut app, width, HEIGHT);
        app.open_display_panes(Instant::now());
        layout_sized(&mut app, width, HEIGHT);

        let (row, _) = mode_bar_row(&app, width, HEIGHT);
        assert!(row.ends_with("any key close"), "{row:?}");
        assert!(!row.contains("VERSION"), "{row:?}");
    }

    /// #118: while the labels are up the whole mode is red, like the
    /// close-workspace confirmation: the bar's chips and keys, the focused
    /// pane's border, and a muted red on the other borders. Closing the labels
    /// puts every colour back.
    #[test]
    fn the_labels_mode_is_red_and_closing_it_restores_the_colours() {
        let mut app = two_pane_app();
        let normal = (app.pane_border_color(true), app.pane_border_color(false));
        app.open_display_panes(Instant::now());
        layout_sized(&mut app, WIDTH, HEIGHT);
        let red = app.palette.red;
        let muted = app.palette.muted_red();
        assert_ne!(muted, red, "the inactive red is a different, muted red");

        assert_eq!(app.pane_border_color(true), red);
        assert_eq!(app.pane_border_color(false), muted);

        let (row, buffer) = mode_bar_row(&app, WIDTH, HEIGHT);
        let col = |text: &str| row[..row.find(text).expect("on the bar")].chars().count() as u16;
        let at = |text: &str| buffer[(col(text), HEIGHT - 1)].style();
        assert_eq!(at("PANES").bg, Some(red));
        assert_eq!(at("VERSION").bg, Some(red));
        assert_eq!(at("any key").fg, Some(red));
        assert_eq!(at("1-2").fg, Some(red));

        // The drawn borders follow: across the panes' area there are red
        // (focused) and muted red (other) border cells, and none left in the
        // normal focused colour.
        let area = app.view.terminal_area;
        let fgs: Vec<_> = (area.y..area.y + area.height)
            .flat_map(|y| (area.x..area.x + area.width).map(move |x| (x, y)))
            .filter(|(x, y)| {
                matches!(
                    buffer[(*x, *y)].symbol(),
                    "│" | "─" | "┌" | "┐" | "└" | "┘" | "├" | "┤" | "┬" | "┴" | "┼"
                )
            })
            .filter_map(|(x, y)| buffer[(x, y)].style().fg)
            .collect();
        assert!(fgs.contains(&red), "a red border cell");
        assert!(fgs.contains(&muted), "a muted red border cell");
        assert!(
            !fgs.contains(&normal.0),
            "no border left in the normal colour"
        );

        app.close_overlay(crate::app::state::OverlayKind::DisplayPanes);
        assert_eq!(
            (app.pane_border_color(true), app.pane_border_color(false)),
            normal
        );
    }

    /// #118: each sidebar section shows its own size on its bottom row while
    /// the labels are up.
    #[test]
    fn each_sidebar_section_shows_its_size_while_the_labels_are_up() {
        let mut app = two_pane_app();
        app.open_display_panes(Instant::now());
        layout_sized(&mut app, WIDTH, HEIGHT);
        let buffer = draw_sized(&app, WIDTH, HEIGHT);
        let (spaces, agents) =
            crate::ui::expanded_sidebar_sections(app.view.sidebar_rect, app.sidebar_section_split);

        for section in [spaces, agents] {
            assert!(section.width > 0 && section.height > 0);
            let bottom = Rect::new(section.x, section.y + section.height - 1, section.width, 1);
            let text = crate::ui::test_support::row_text(&buffer, bottom);
            let size = format!("{}x{}", section.width, section.height);
            assert!(text.contains(&size), "{size} on {text:?}");
            let col = bottom.x + text.find(&size).expect("size") as u16;
            assert_eq!(buffer[(col, bottom.y)].style().bg, Some(app.palette.red));
        }
    }

    #[test]
    fn a_collapsed_sidebar_shows_no_section_sizes() {
        let mut app = two_pane_app();
        app.sidebar_collapsed = true;
        layout_sized(&mut app, WIDTH, HEIGHT);
        app.open_display_panes(Instant::now());
        layout_sized(&mut app, WIDTH, HEIGHT);
        let rows = screen(&app);
        let sidebar = app.view.sidebar_rect;
        // A size is digits, `x`, digits.
        let has_size = |text: &str| {
            let chars: Vec<char> = text.chars().collect();
            chars
                .windows(3)
                .any(|w| w[0].is_ascii_digit() && w[1] == 'x' && w[2].is_ascii_digit())
        };
        for row in &rows {
            let left: String = row.chars().take(usize::from(sidebar.width)).collect();
            assert!(
                !has_size(&left),
                "no size in the collapsed sidebar: {left:?}"
            );
        }
    }

    #[test]
    fn snapshot_display_panes() {
        overlay_snapshot_of(|app| app.open_display_panes(Instant::now())).assert(
            Rect::new(19, 11, 60, 14),
            &[
                "      │             ┌──────────────────────────┐",
                "25x13 │             │ 1  w2:p1 · pane 1  54x24 │",
                "──────│             └──────────────────────────┘",
                "rouped│",
                "      │",
                "      │",
                "      │",
                "      │",
                "      │",
                "      │",
                "      │",
                "      │",
                "      │",
                "25x12 │ PANES   window 80x25 · panes 54x24  1 focus  any key",
            ],
        );
    }
}
