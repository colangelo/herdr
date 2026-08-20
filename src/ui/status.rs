use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
    Frame,
};

use super::text::display_width_u16;
use super::widgets::panel_contrast_fg;
use crate::{
    app::state::{CopyFeedback, Palette, ToastKind, ToastNotification},
    config::{ToastClipboardPosition, ToastHerdrPosition, ToastHerdrSize},
    detect::AgentState,
};

/// Inner padding (columns, rows) added inside the toast box for each size.
fn toast_size_padding(size: ToastHerdrSize) -> (u16, u16) {
    match size {
        ToastHerdrSize::Auto => (0, 0),
        ToastHerdrSize::Medium | ToastHerdrSize::Large => (2, 1),
    }
}

/// Minimum toast width for a size, relative to the anchor area.
fn toast_size_min_width(size: ToastHerdrSize, anchor_width: u16) -> u16 {
    match size {
        ToastHerdrSize::Auto => 0,
        ToastHerdrSize::Medium => anchor_width * 2 / 5,
        ToastHerdrSize::Large => anchor_width * 3 / 5,
    }
}

pub(crate) fn copy_feedback_rect(
    area: Rect,
    feedback: &CopyFeedback,
    offset_rows: u16,
    position: ToastClipboardPosition,
) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }

    let content_width = feedback.message.len() as u16 + 4;
    let width = content_width.min(area.width);
    let height = 3u16.min(area.height);
    let x = match position {
        ToastClipboardPosition::TopLeft | ToastClipboardPosition::BottomLeft => area.x,
        ToastClipboardPosition::TopCenter | ToastClipboardPosition::BottomCenter => {
            area.x + area.width.saturating_sub(width) / 2
        }
        ToastClipboardPosition::TopRight | ToastClipboardPosition::BottomRight => {
            area.x + area.width.saturating_sub(width)
        }
    };
    let y = match position {
        ToastClipboardPosition::TopLeft
        | ToastClipboardPosition::TopCenter
        | ToastClipboardPosition::TopRight => area.y + offset_rows.min(area.height),
        ToastClipboardPosition::BottomLeft
        | ToastClipboardPosition::BottomCenter
        | ToastClipboardPosition::BottomRight => {
            area.y + area.height.saturating_sub(height + offset_rows)
        }
    };
    Rect::new(x, y, width, height)
}

pub(crate) fn toast_notification_rect(
    area: Rect,
    content_area: Rect,
    toast: &ToastNotification,
    offset_for_warning: bool,
    position: ToastHerdrPosition,
    size: ToastHerdrSize,
) -> Rect {
    // Corner toasts anchor to the full frame; the centered toast anchors to
    // the pane content area so it floats between the panes, not over the
    // sidebar.
    let anchor = if position == ToastHerdrPosition::Center && !content_area.is_empty() {
        content_area
    } else {
        area
    };
    let (pad_cols, pad_rows) = toast_size_padding(size);
    let content_width = display_width_u16(&toast.title)
        .max(display_width_u16(&toast.context))
        .saturating_add(4);
    let width = content_width
        .saturating_add(2 + pad_cols * 2)
        .max(toast_size_min_width(size, anchor.width))
        .min(anchor.width);
    let content_height: u16 = if toast.context.is_empty() { 1 } else { 2 };
    let height = (content_height + 2 + pad_rows * 2).min(anchor.height);
    let x = match position {
        ToastHerdrPosition::TopLeft | ToastHerdrPosition::BottomLeft => anchor.x,
        ToastHerdrPosition::TopRight | ToastHerdrPosition::BottomRight => {
            anchor.x + anchor.width.saturating_sub(width)
        }
        ToastHerdrPosition::Center => anchor.x + anchor.width.saturating_sub(width) / 2,
    };
    let warning_offset = u16::from(offset_for_warning);
    let y = match position {
        ToastHerdrPosition::TopLeft | ToastHerdrPosition::TopRight => {
            anchor.y + warning_offset.min(anchor.height)
        }
        ToastHerdrPosition::BottomLeft | ToastHerdrPosition::BottomRight => {
            anchor.y + anchor.height.saturating_sub(height + warning_offset)
        }
        // The centered toast floats mid-area, unaffected by the warning row.
        ToastHerdrPosition::Center => anchor.y + anchor.height.saturating_sub(height) / 2,
    };
    Rect::new(x, y, width, height)
}

pub(super) fn render_toast_notification(
    frame: &mut Frame,
    area: Rect,
    content_area: Rect,
    toast: &ToastNotification,
    offset_for_warning: bool,
    position: ToastHerdrPosition,
    size: ToastHerdrSize,
    p: &Palette,
) {
    let dot_color = match toast.kind {
        ToastKind::NeedsAttention => p.red,
        ToastKind::Finished => p.blue,
        ToastKind::UpdateInstalled => p.accent,
    };
    let toast_area = toast_notification_rect(
        area,
        content_area,
        toast,
        offset_for_warning,
        position,
        size,
    );

    frame.render_widget(Clear, toast_area);
    let (pad_cols, pad_rows) = toast_size_padding(size);
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::new(pad_cols, pad_cols, pad_rows, pad_rows))
        .border_style(Style::default().fg(p.overlay0))
        .style(Style::default().bg(p.panel_bg));
    let inner = block.inner(toast_area);
    frame.render_widget(block, toast_area);

    if inner.height < 1 {
        return;
    }

    let [title_row, context_row] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);

    let title = Line::from(vec![
        Span::styled("●", Style::default().fg(dot_color)),
        Span::raw(" "),
        Span::styled(
            &toast.title,
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ),
    ]);
    let context = Line::from(vec![
        Span::styled("  ", Style::default().fg(p.overlay0)),
        Span::styled(&toast.context, Style::default().fg(p.overlay0)),
    ]);

    frame.render_widget(Paragraph::new(title), title_row);
    if !toast.context.is_empty() && inner.height >= 2 {
        frame.render_widget(Paragraph::new(context), context_row);
    }
}

pub(super) fn render_copy_feedback(
    frame: &mut Frame,
    area: Rect,
    feedback: &CopyFeedback,
    offset_rows: u16,
    position: ToastClipboardPosition,
    p: &Palette,
) {
    let feedback_area = copy_feedback_rect(area, feedback, offset_rows, position);
    if feedback_area.is_empty() {
        return;
    }

    frame.render_widget(Clear, feedback_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.green))
        .style(Style::default().bg(p.panel_bg));
    let inner = block.inner(feedback_area);
    frame.render_widget(block, feedback_area);

    if inner.height == 0 {
        return;
    }

    let text = Line::from(vec![
        Span::styled("●", Style::default().fg(p.green).bg(p.panel_bg)),
        Span::raw(" "),
        Span::styled(
            &feedback.message,
            Style::default()
                .fg(p.text)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(text), inner);
}

pub(super) fn render_config_diagnostic(frame: &mut Frame, area: Rect, message: &str, p: &Palette) {
    let style = Style::default()
        .fg(panel_contrast_fg(p))
        .bg(p.yellow)
        .add_modifier(Modifier::BOLD);

    for (row, line) in message
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(area.height as usize)
        .enumerate()
    {
        let text = format!(" {line} ");
        let width = (text.len() as u16).min(area.width);
        let notif_area = Rect::new(
            area.x + area.width.saturating_sub(width),
            area.y + row as u16,
            width,
            1,
        );

        frame.render_widget(Clear, notif_area);
        frame.render_widget(Paragraph::new(Span::styled(text, style)), notif_area);
    }
}

pub(super) fn state_icon<'a>(
    state: AgentState,
    seen: bool,
    symbols: &crate::app::state::StateIconSymbols<'a>,
    colors: &crate::app::state::StateIconColors,
) -> (&'a str, Style) {
    (
        symbols.symbol(state, seen),
        Style::default().fg(state_label_color(state, seen, colors)),
    )
}

/// Herdr's working spinner, stepped by the agent's own title activity rather
/// than a timer: frame `n` is `ACTIVITY_FRAMES[n % 10]`.
const ACTIVITY_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The icon for an agent row: the working glyph animates when the agent's
/// title spinner has ticked (`activity_frame`) and the spinner is enabled;
/// every other state, and a working agent with a still title, falls through
/// to [`state_icon`].
pub(super) fn agent_state_icon<'a>(
    state: AgentState,
    seen: bool,
    activity_frame: Option<u8>,
    spinner: crate::config::StatusSpinnerConfig,
    symbols: &crate::app::state::StateIconSymbols<'a>,
    colors: &crate::app::state::StateIconColors,
) -> (&'a str, Style) {
    let (glyph, style) = state_icon(state, seen, symbols, colors);
    match (state, spinner, activity_frame) {
        (AgentState::Working, crate::config::StatusSpinnerConfig::Agent, Some(frame)) => (
            ACTIVITY_FRAMES[usize::from(frame) % ACTIVITY_FRAMES.len()],
            style,
        ),
        _ => (glyph, style),
    }
}

pub(super) fn state_label(state: AgentState, seen: bool) -> &'static str {
    match (state, seen) {
        (AgentState::Blocked, _) => "blocked",
        (AgentState::Working, _) => "working",
        (AgentState::Idle, false) => "done",
        (AgentState::Idle, true) => "idle",
        (AgentState::Unknown, _) => "idle",
    }
}

pub(super) fn state_label_color(
    state: AgentState,
    seen: bool,
    colors: &crate::app::state::StateIconColors,
) -> Color {
    match (state, seen) {
        (AgentState::Blocked, _) => colors.blocked,
        (AgentState::Working, _) => colors.working,
        (AgentState::Idle, false) => colors.done,
        (AgentState::Idle, true) => colors.idle,
        (AgentState::Unknown, _) => colors.unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatusIndicatorStyle;
    use crate::config::{ToastClipboardPosition, ToastHerdrPosition};

    fn toast() -> ToastNotification {
        ToastNotification {
            kind: ToastKind::Finished,
            title: "done".to_string(),
            context: "workspace".to_string(),
            position: None,
            target: None,
        }
    }

    fn feedback() -> CopyFeedback {
        CopyFeedback {
            message: "copied to clipboard".to_string(),
        }
    }

    #[test]
    fn agent_state_icon_animates_only_a_working_agent_with_a_live_title_spinner() {
        use crate::config::StatusSpinnerConfig;
        let palette = Palette::catppuccin();
        let colors = crate::app::state::StateIconColors {
            working: palette.yellow,
            idle: palette.green,
            done: palette.teal,
            blocked: palette.red,
            unknown: palette.overlay0,
        };
        let symbols = crate::app::state::StateIconSymbols::for_style(StatusIndicatorStyle::Symbols);
        let icon = |state, seen, frame, spinner| {
            agent_state_icon(state, seen, frame, spinner, &symbols, &colors)
        };

        let (glyph, style) = icon(
            AgentState::Working,
            true,
            Some(3),
            StatusSpinnerConfig::Agent,
        );
        assert_eq!(glyph, "⠸");
        assert_eq!(style.fg, Some(palette.yellow));
        assert_eq!(display_width_u16(glyph), 1);
        // Frames wrap around the ten-cell snake.
        assert_eq!(
            icon(
                AgentState::Working,
                true,
                Some(13),
                StatusSpinnerConfig::Agent
            )
            .0,
            "⠸"
        );
        // A still title keeps the static working glyph.
        assert_eq!(
            icon(AgentState::Working, true, None, StatusSpinnerConfig::Agent).0,
            "◐"
        );
        // Off always draws the static glyph.
        assert_eq!(
            icon(AgentState::Working, true, Some(3), StatusSpinnerConfig::Off).0,
            "◐"
        );
        // Other states ignore the frame entirely.
        assert_eq!(
            icon(AgentState::Idle, false, Some(3), StatusSpinnerConfig::Agent).0,
            "□"
        );
        assert_eq!(
            icon(
                AgentState::Blocked,
                true,
                Some(3),
                StatusSpinnerConfig::Agent
            )
            .0,
            "×"
        );
    }

    #[test]
    fn state_icons_support_dot_and_distinct_symbol_styles() {
        let palette = Palette::catppuccin();
        let colors = crate::app::state::StateIconColors {
            working: palette.yellow,
            idle: palette.green,
            done: palette.teal,
            blocked: palette.red,
            unknown: palette.overlay0,
        };
        for (indicator_style, expected_symbols) in [
            (StatusIndicatorStyle::Dots, ["●", "●", "●", "○", "·"]),
            (StatusIndicatorStyle::Symbols, ["×", "◐", "□", "✓", "·"]),
        ] {
            let symbols = crate::app::state::StateIconSymbols::for_style(indicator_style);
            for ((state, seen, color), expected_symbol) in [
                (AgentState::Blocked, true, palette.red),
                (AgentState::Working, true, palette.yellow),
                (AgentState::Idle, false, palette.teal),
                (AgentState::Idle, true, palette.green),
                (AgentState::Unknown, true, palette.overlay0),
            ]
            .into_iter()
            .zip(expected_symbols)
            {
                let (actual_symbol, style) = state_icon(state, seen, &symbols, &colors);
                assert_eq!(actual_symbol, expected_symbol);
                assert_eq!(display_width_u16(actual_symbol), 1);
                assert_eq!(style.fg, Some(color));
            }
        }
    }

    #[test]
    fn toast_rect_uses_configured_corner() {
        let area = Rect::new(10, 20, 100, 40);
        let content = Rect::new(40, 22, 70, 38);
        let toast = toast();

        let top_left = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::TopLeft,
            ToastHerdrSize::Auto,
        );
        assert_eq!(top_left.x, area.x);
        assert_eq!(top_left.y, area.y);

        let top_right = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::TopRight,
            ToastHerdrSize::Auto,
        );
        assert_eq!(top_right.x + top_right.width, area.x + area.width);
        assert_eq!(top_right.y, area.y);

        let bottom_left = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::BottomLeft,
            ToastHerdrSize::Auto,
        );
        assert_eq!(bottom_left.x, area.x);
        assert_eq!(bottom_left.y + bottom_left.height, area.y + area.height);

        let bottom_right = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::BottomRight,
            ToastHerdrSize::Auto,
        );
        assert_eq!(bottom_right.x + bottom_right.width, area.x + area.width);
        assert_eq!(bottom_right.y + bottom_right.height, area.y + area.height);
    }

    #[test]
    fn toast_rect_center_anchors_to_content_area_and_ignores_warning_offset() {
        let area = Rect::new(0, 0, 120, 40);
        let content = Rect::new(30, 2, 90, 38);
        let toast = toast();

        let center = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::Center,
            ToastHerdrSize::Auto,
        );
        assert_eq!(center.x, content.x + (content.width - center.width) / 2);
        assert_eq!(center.y, content.y + (content.height - center.height) / 2);

        let with_warning = toast_notification_rect(
            area,
            content,
            &toast,
            true,
            ToastHerdrPosition::Center,
            ToastHerdrSize::Auto,
        );
        assert_eq!(with_warning, center);

        // With no content area (e.g. empty view), center falls back to the frame.
        let fallback = toast_notification_rect(
            area,
            Rect::default(),
            &toast,
            false,
            ToastHerdrPosition::Center,
            ToastHerdrSize::Auto,
        );
        assert_eq!(fallback.x, area.x + (area.width - fallback.width) / 2);
        assert_eq!(fallback.y, area.y + (area.height - fallback.height) / 2);
    }

    #[test]
    fn toast_rect_size_presets_widen_and_pad_the_box() {
        let area = Rect::new(0, 0, 120, 40);
        let content = Rect::new(20, 0, 100, 40);
        let toast = toast();

        let auto = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::Center,
            ToastHerdrSize::Auto,
        );
        let medium = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::Center,
            ToastHerdrSize::Medium,
        );
        let large = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::Center,
            ToastHerdrSize::Large,
        );

        // 40% / 60% of the 100-col content anchor.
        assert_eq!(medium.width, 40);
        assert_eq!(large.width, 60);
        assert!(auto.width < medium.width);
        // One padding row above and below the two content rows.
        assert_eq!(medium.height, auto.height + 2);
        assert_eq!(large.height, auto.height + 2);

        // Corner toasts size against the full frame instead.
        let corner_large = toast_notification_rect(
            area,
            content,
            &toast,
            false,
            ToastHerdrPosition::BottomRight,
            ToastHerdrSize::Large,
        );
        assert_eq!(corner_large.width, 72);
    }

    #[test]
    fn toast_rect_uses_display_width_for_cjk_labels() {
        let area = Rect::new(0, 0, 100, 20);
        let toast = ToastNotification {
            kind: ToastKind::NeedsAttention,
            title: "重构用户认证模块".to_string(),
            context: "提交 herdr 的反馈".to_string(),
            position: None,
            target: None,
        };

        let rect = toast_notification_rect(
            area,
            Rect::default(),
            &toast,
            false,
            ToastHerdrPosition::TopRight,
            ToastHerdrSize::Auto,
        );

        let expected_content_width =
            display_width_u16(&toast.title).max(display_width_u16(&toast.context)) + 6;
        assert_eq!(rect.width, expected_content_width);
        assert_eq!(rect.x + rect.width, area.x + area.width);
    }

    #[test]
    fn copy_feedback_rect_uses_configured_position() {
        let area = Rect::new(10, 20, 100, 40);
        let feedback = feedback();

        let top_center = copy_feedback_rect(area, &feedback, 0, ToastClipboardPosition::TopCenter);
        assert_eq!(top_center.y, area.y);
        assert_eq!(
            top_center.x,
            area.x + area.width.saturating_sub(top_center.width) / 2
        );

        let bottom_center =
            copy_feedback_rect(area, &feedback, 0, ToastClipboardPosition::BottomCenter);
        assert_eq!(bottom_center.y + bottom_center.height, area.y + area.height);
        assert_eq!(
            bottom_center.x,
            area.x + area.width.saturating_sub(bottom_center.width) / 2
        );
    }
}
