use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::text::{display_width, relative_time_label, truncate_end};
use super::widgets::{
    action_button_row_rects, panel_contrast_fg, render_action_button, render_panel_shell,
    ActionButtonSpec,
};
use crate::app::state::{NotificationCenterButton, ToastKind};
use crate::app::AppState;

/// Footer buttons in the settings-panel language: the shortcut hint inside
/// the filled box, in render order.
const MARK_READ_BUTTON: (&str, &str) = ("r", "mark read");
const CLEAR_BUTTON: (&str, &str) = ("c", "clear all");
const CLOSE_BUTTON: (&str, &str) = ("esc", "close");

fn button_spec(button: (&'static str, &'static str)) -> ActionButtonSpec<'static> {
    ActionButtonSpec {
        hint: Some(button.0),
        label: button.1,
    }
}

/// Footer button rects; the mouse layer and the render agree on this
/// geometry. `mark_read` is dropped first when the panel is too narrow for
/// all three boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotificationCenterButtonRects {
    pub mark_read: Option<Rect>,
    pub clear: Rect,
    pub close: Rect,
}

impl NotificationCenterButtonRects {
    pub(crate) fn hit(&self, col: u16, row: u16) -> Option<NotificationCenterButton> {
        let contains = |rect: Rect| col >= rect.x && col < rect.x + rect.width && row == rect.y;
        if self.mark_read.is_some_and(contains) {
            return Some(NotificationCenterButton::MarkRead);
        }
        if contains(self.clear) {
            return Some(NotificationCenterButton::Clear);
        }
        if contains(self.close) {
            return Some(NotificationCenterButton::Close);
        }
        None
    }

    pub(crate) fn row_y(&self) -> u16 {
        self.clear.y
    }
}

/// Compute the footer button row for the panel's inner rect: the settings
/// buttons' centered row layout on the last inner row.
pub(crate) fn notification_center_button_rects(
    inner: Rect,
) -> Option<NotificationCenterButtonRects> {
    // Needs room for the whole footer block, blank row included, or the
    // buttons would sit flush against the last notification.
    if inner.width == 0 || inner.height < super::widgets::FOOTER_ROWS {
        return None;
    }
    let gap = 2u16;
    let all = [
        button_spec(MARK_READ_BUTTON),
        button_spec(CLEAR_BUTTON),
        button_spec(CLOSE_BUTTON),
    ];
    let all_width: u16 = all
        .iter()
        .map(|spec| super::widgets::action_button_width(spec.hint, spec.label))
        .sum::<u16>()
        + gap * 2;
    let with_mark_read = all_width <= inner.width;
    let row_offset = inner.height - 1;
    if with_mark_read {
        let rects = action_button_row_rects(inner, &all, gap, row_offset);
        Some(NotificationCenterButtonRects {
            mark_read: Some(rects[0]),
            clear: rects[1],
            close: rects[2],
        })
    } else {
        let rects = action_button_row_rects(inner, &all[1..], gap, row_offset);
        Some(NotificationCenterButtonRects {
            mark_read: None,
            clear: rects[0],
            close: rects[1],
        })
    }
}

/// Hit area for the floating indicator used when
/// `ui.notification_center_position = "bottom-right"`: the last row of the
/// frame, right-aligned, mirroring where the dropdown opens.
pub(super) fn floating_notification_indicator_rect(area: Rect, width: u16) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }
    let width = width.min(area.width);
    Rect::new(
        area.x + area.width - width,
        area.y + area.height - 1,
        width,
        1,
    )
}

/// Draw the bottom-right floating indicator. A no-op for the default
/// top-right position, where the indicator lives in the tab bar instead.
pub(super) fn render_floating_notification_indicator(app: &AppState, frame: &mut Frame) {
    if app.notification_center_position
        != crate::config::NotificationCenterPositionConfig::BottomRight
    {
        return;
    }
    let rect = app.view.notification_hit_area;
    if rect.width == 0 {
        return;
    }
    let p = &app.palette;
    let unread = app.notification_log.unread_count();
    // Same quiet grammar as the sidebar's « collapse toggle: a bare glyph
    // with no background, dim at rest and accent + bold while unread.
    let style = if unread > 0 {
        Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.overlay0)
    };
    frame.render_widget(
        Paragraph::new(super::tabs::notification_indicator_label(unread)).style(style),
        rect,
    );
}

pub(super) fn render_notification_center(app: &AppState, frame: &mut Frame) {
    let Some(rect) = app.notification_center_rect() else {
        return;
    };
    let p = &app.palette;
    let Some(inner) = render_panel_shell(frame, rect, p.accent, p.panel_bg) else {
        return;
    };

    if app.notification_log.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " no notifications",
                Style::default().fg(p.overlay0),
            ))),
            Rect::new(inner.x, inner.y, inner.width, inner.height.min(1)),
        );
        return;
    }

    let Some((list, start)) = app.notification_center_list_window() else {
        return;
    };
    let selected = app
        .notification_center
        .as_ref()
        .map(|center| center.selected)
        .unwrap_or(0);
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    for (row, entry) in app
        .notification_log
        .entries_newest_first()
        .skip(start)
        .take(list.height as usize)
        .enumerate()
    {
        let idx = start + row;
        let row_rect = Rect::new(list.x, list.y + row as u16, list.width, 1);
        let is_selected = idx == selected;

        let age = relative_time_label(now_unix, entry.posted_at_unix);
        let age_width = display_width(&age);
        let text_budget = (list.width as usize).saturating_sub(3 + age_width + 1);
        let title = truncate_end(&entry.title, text_budget);
        let context_budget = text_budget.saturating_sub(display_width(&title));
        let context = if entry.context.is_empty() || context_budget < 6 {
            String::new()
        } else {
            truncate_end(&format!(" · {}", entry.context), context_budget)
        };
        let pad_width = (list.width as usize)
            .saturating_sub(3 + display_width(&title) + display_width(&context) + age_width + 1);

        let (dot_style, title_style, dim_style, row_style) = if is_selected {
            let fg = panel_contrast_fg(p);
            let selected_base = Style::default().fg(fg).bg(p.accent);
            // The band alone marks selection; dot and bold keep signalling
            // read state so a selected row stays distinguishable.
            let selected_title = if entry.read {
                selected_base
            } else {
                selected_base.add_modifier(Modifier::BOLD)
            };
            (selected_base, selected_title, selected_base, selected_base)
        } else if !entry.read {
            // Unread: kind-colored dot, bold title.
            let dot_color = match entry.kind {
                ToastKind::NeedsAttention => p.red,
                ToastKind::Finished => p.blue,
                ToastKind::UpdateInstalled => p.accent,
            };
            (
                Style::default().fg(dot_color),
                Style::default().fg(p.text).add_modifier(Modifier::BOLD),
                Style::default().fg(p.overlay0),
                Style::default(),
            )
        } else {
            // Read: blank dot column, dim regular-weight title.
            (
                Style::default(),
                Style::default().fg(p.overlay0),
                Style::default().fg(p.overlay0),
                Style::default(),
            )
        };
        let dot = if !entry.read { " ● " } else { "   " };

        let line = Line::from(vec![
            Span::styled(dot, dot_style),
            Span::styled(title, title_style),
            Span::styled(context, dim_style),
            Span::styled(" ".repeat(pad_width), row_style),
            Span::styled(age, dim_style),
            Span::styled(" ", row_style),
        ]);
        frame.render_widget(Paragraph::new(line).style(row_style), row_rect);
    }

    if let Some(buttons) = app.notification_center_buttons() {
        let hovered = app
            .notification_center
            .as_ref()
            .and_then(|center| center.hovered_button);
        // Same button language as the settings action buttons: filled boxes
        // with the shortcut hint inside, secondary (surface) at rest and
        // accent on hover.
        let style_for = |button: NotificationCenterButton| {
            if hovered == Some(button) {
                Style::default()
                    .fg(panel_contrast_fg(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(p.text)
                    .bg(p.surface0)
                    .add_modifier(Modifier::BOLD)
            }
        };
        if let Some(mark_read_rect) = buttons.mark_read {
            render_action_button(
                frame,
                mark_read_rect,
                Some(MARK_READ_BUTTON.0),
                MARK_READ_BUTTON.1,
                style_for(NotificationCenterButton::MarkRead),
            );
        }
        render_action_button(
            frame,
            buttons.clear,
            Some(CLEAR_BUTTON.0),
            CLEAR_BUTTON.1,
            style_for(NotificationCenterButton::Clear),
        );
        render_action_button(
            frame,
            buttons.close,
            Some(CLOSE_BUTTON.0),
            CLOSE_BUTTON.1,
            style_for(NotificationCenterButton::Close),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{AppState, ToastKind, ToastNotification};
    use ratatui::layout::Rect;
    use ratatui::{backend::TestBackend, Terminal};

    fn toast(title: &str) -> ToastNotification {
        ToastNotification {
            kind: ToastKind::Finished,
            title: title.to_string(),
            context: String::new(),
            position: None,
            target: None,
        }
    }

    #[test]
    fn floating_indicator_rect_hugs_the_bottom_right_corner() {
        let area = Rect::new(0, 0, 80, 25);
        let rect = floating_notification_indicator_rect(area, 5);
        assert_eq!(rect, Rect::new(75, 24, 5, 1));

        assert_eq!(
            floating_notification_indicator_rect(Rect::default(), 5),
            Rect::default()
        );
    }

    #[test]
    fn floating_indicator_renders_bare_accent_glyph_at_bottom_right() {
        let mut app = AppState::test_new();
        app.notification_center_position =
            crate::config::NotificationCenterPositionConfig::BottomRight;
        app.post_notification(toast("one"));
        app.post_notification(toast("two"));
        let area = Rect::new(0, 0, 80, 25);
        app.view.notification_hit_area = floating_notification_indicator_rect(
            area,
            super::super::tabs::notification_indicator_width(app.notification_log.unread_count()),
        );

        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_floating_notification_indicator(&app, frame))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let rect = app.view.notification_hit_area;
        assert_eq!(rect.y, 24, "indicator sits on the frame's last row");
        assert_eq!(rect.x + rect.width, 80, "right-aligned to the frame edge");
        let row: String = (rect.x..rect.x + rect.width)
            .map(|x| buffer[(x, rect.y)].symbol())
            .collect();
        assert!(row.contains("◆ 2"), "indicator row: {row:?}");
        let mid = &buffer[(rect.x + 1, rect.y)];
        assert_eq!(
            mid.style().fg,
            Some(app.palette.accent),
            "unread glyph uses the accent color"
        );
        assert_ne!(
            mid.style().bg,
            Some(app.palette.accent),
            "no filled pill: the glyph stays bare like the sidebar toggle"
        );
    }

    #[test]
    fn footer_renders_settings_style_buttons_above_the_bottom_border() {
        let mut app = AppState::test_new();
        app.view.terminal_area = Rect::new(0, 1, 80, 24);
        app.view.tab_bar_rect = Rect::new(0, 0, 80, 1);
        // A long title widens the panel enough for all three footer buttons.
        app.post_notification(toast("claude finished a very long-running task"));
        app.post_notification(toast("two"));
        app.open_notification_center();

        let panel = app.notification_center_rect().expect("panel rect");
        let buttons = app.notification_center_buttons().expect("footer buttons");

        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_notification_center(&app, frame))
            .unwrap();
        let buffer = terminal.backend().buffer();

        // The buttons sit on the row directly above the panel's bottom border.
        assert_eq!(buttons.clear.y, panel.y + panel.height - 2);

        // And the row between the last entry and the buttons renders blank.
        // Geometry alone would not catch a stray draw into the separator, so
        // this reads the buffer rather than the rects.
        let (list, _) = app.notification_center_list_window().expect("list window");
        let separator_y = list.y + list.height;
        assert_eq!(
            separator_y + 1,
            buttons.clear.y,
            "separator precedes buttons"
        );
        let separator: String = (list.x..list.x + list.width)
            .map(|x| buffer[(x, separator_y)].symbol())
            .collect();
        assert!(
            separator.trim().is_empty(),
            "the row above the buttons should be blank, got {separator:?}"
        );

        // Each carries the settings-style filled background (surface0 at
        // rest) with the shortcut hint inside the box.
        for (rect, text) in [
            (buttons.mark_read.expect("mark-read fits"), "r mark read"),
            (buttons.clear, "c clear all"),
            (buttons.close, "esc close"),
        ] {
            let mid = &buffer[(rect.x + rect.width / 2, rect.y)];
            assert_eq!(mid.style().bg, Some(app.palette.surface0));
            let row: String = (rect.x..rect.x + rect.width)
                .map(|x| buffer[(x, rect.y)].symbol())
                .collect();
            assert!(row.contains(text), "button row {text:?}: {row:?}");
        }
    }

    #[test]
    fn unread_rows_show_dot_and_bold_while_read_rows_dim() {
        let mut app = AppState::test_new();
        app.view.terminal_area = Rect::new(0, 1, 80, 24);
        app.view.tab_bar_rect = Rect::new(0, 0, 80, 1);
        app.post_notification(toast("older"));
        app.post_notification(toast("newer"));
        let older_id = app
            .notification_log
            .entries_newest_first()
            .last()
            .expect("older entry")
            .id;
        app.notification_log.mark_read(older_id);
        app.open_notification_center();
        // Move selection off both rows under test? The panel always has a
        // selection; keep it on row 0 and inspect row 1 (read) plus row 0's
        // selected styling separately via the unselected unread row below.
        app.notification_center_move_selection(1);
        // Selection now sits on the read row (row 1); row 0 is the plain
        // unread rendering.
        let (list, _start) = app
            .notification_center_list_window()
            .expect("list window present");

        let backend = TestBackend::new(80, 25);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_notification_center(&app, frame))
            .unwrap();
        let buffer = terminal.backend().buffer();

        // Unread row (unselected): kind-colored dot and a bold title.
        let dot = &buffer[(list.x + 1, list.y)];
        assert_eq!(dot.symbol(), "●");
        assert_eq!(dot.style().fg, Some(app.palette.blue));
        let title_cell = &buffer[(list.x + 3, list.y)];
        assert!(title_cell
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD));
        assert_eq!(title_cell.style().fg, Some(app.palette.text));

        // The read row is selected here: the band marks the selection, but
        // the dot stays absent and the title stays regular-weight so read
        // state remains visible on the selected row.
        let selected_read_dot = &buffer[(list.x + 1, list.y + 1)];
        assert_eq!(
            selected_read_dot.symbol(),
            " ",
            "selected read rows still drop the dot"
        );
        assert_eq!(selected_read_dot.style().bg, Some(app.palette.accent));
        let selected_read_title = &buffer[(list.x + 3, list.y + 1)];
        assert!(
            !selected_read_title
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD),
            "selected read rows stay regular-weight"
        );

        // With the selection moved back to the unread row, the read row keeps
        // its blank-dot/dim rendering and the selected unread row keeps its
        // dot and bold on the band.
        app.notification_center_move_selection(-1);
        terminal
            .draw(|frame| render_notification_center(&app, frame))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let selected_unread_dot = &buffer[(list.x + 1, list.y)];
        assert_eq!(
            selected_unread_dot.symbol(),
            "●",
            "selected unread rows keep the dot"
        );
        let read_dot = &buffer[(list.x + 1, list.y + 1)];
        assert_eq!(read_dot.symbol(), " ", "read rows drop the dot");
        let read_title = &buffer[(list.x + 3, list.y + 1)];
        assert_eq!(
            read_title.style().fg,
            Some(app.palette.overlay0),
            "read titles dim to the muted gray"
        );
        assert!(!read_title
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD));
    }
}
