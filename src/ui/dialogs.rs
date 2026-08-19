use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};

use super::text::{display_width_u16, truncate_end};
use super::text_field::TextField;
use super::widgets::{
    action_button_row_rects, centered_popup_rect, footer_split, panel_contrast_fg,
    render_action_button, render_modal_header, render_modal_shell, render_panel_shell,
    ActionButtonSpec, HEADER_ROWS,
};
use crate::app::{state::WorktreeOpenState, AppState, Mode};
use crate::terminal::TerminalRuntimeRegistry;

const NEW_LINKED_WORKTREE_POPUP_WIDTH: u16 = 68;
const NEW_LINKED_WORKTREE_POPUP_HEIGHT: u16 = 13;

pub(crate) fn rename_button_rects(inner: Rect) -> (Rect, Rect, Rect) {
    let rects = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("↵"),
                label: "save",
            },
            ActionButtonSpec {
                hint: Some("^c"),
                label: "clear",
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: "cancel",
            },
        ],
        2,
        3,
    );
    (rects[0], rects[1], rects[2])
}

/// Draws the shared `name_input` field and puts the host cursor on its caret.
///
/// IMEs draw their composition preview at the host terminal cursor. Without an
/// explicit cursor the frame carries none, the client keeps the position last
/// reported by the focused pane, and composition lands behind the dialog.
fn render_name_input_field(app: &AppState, frame: &mut Frame, input_rect: Rect) {
    frame.render_widget(Clear, input_rect);

    // The text stops one column short of the field so the clamped caret always
    // lands on a blank cell: a host terminal inverts the cell under its cursor,
    // and an IME composes there.
    let text_rect = Rect {
        width: input_rect.width.saturating_sub(1),
        ..input_rect
    };
    frame.render_widget(
        Paragraph::new(format!(" {}", app.name_input.text())).style(
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.surface0),
        ),
        text_rect,
    );

    if input_rect.width == 0 {
        return;
    }
    // The caret follows the field's insertion point rather than the end of
    // the text: the name field has a cursor now.
    let caret_x = input_rect
        .x
        .saturating_add(1)
        .saturating_add(app.name_input.cursor_column().min(u16::MAX as usize) as u16)
        .min(input_rect.right().saturating_sub(1));
    frame.set_cursor_position((caret_x, input_rect.y));
}

pub(crate) const PANE_TODO_EDIT_POPUP_WIDTH: u16 = 60;
pub(crate) const PANE_TODO_EDIT_POPUP_HEIGHT: u16 = 14;

/// How many lines of a todo the modal shows at once. A todo is a note, not a
/// document, so the block is bounded and scrolls rather than growing the modal
/// to fit whatever was pasted into it.
pub(crate) const PANE_TODO_EDIT_INPUT_ROWS: u16 = 3;

/// One column of padding at each edge of the input block, matching the `" "`
/// every other modal row is prefixed with.
const INPUT_PADDING: u16 = 1;

/// The modal's interactive regions. One definition, read by the renderer and
/// by the mouse layer, so clicking "priority" always lands on the row that
/// says "priority".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaneTodoEditRects {
    /// Several rows tall, since a todo may hold more than one line.
    pub input: Rect,
    pub priority: Rect,
    pub link: Rect,
    /// Reserved unconditionally so the geometry does not shift between the
    /// "new" and "edit" modals; only drawn and hit-tested when editing.
    pub done: Rect,
    pub save: Rect,
    pub cancel: Rect,
}

pub(crate) fn pane_todo_edit_rects(inner: Rect) -> Option<PaneTodoEditRects> {
    // The input block spans `PANE_TODO_EDIT_INPUT_ROWS`, pushing everything
    // under it down; `done` is the lowest field row, so the button row
    // (height - 1) must still clear it.
    let below_input = 2 + PANE_TODO_EDIT_INPUT_ROWS;
    if inner.width == 0 || inner.height < below_input + 6 {
        return None;
    }
    let row = |offset: u16| Rect::new(inner.x, inner.y + offset, inner.width, 1);
    let buttons = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("^s"),
                label: "save",
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: "cancel",
            },
        ],
        2,
        inner.height - 1,
    );
    Some(PaneTodoEditRects {
        input: Rect::new(inner.x, inner.y + 2, inner.width, PANE_TODO_EDIT_INPUT_ROWS),
        priority: row(below_input + 1),
        link: row(below_input + 2),
        done: row(below_input + 3),
        save: buttons[0],
        cancel: buttons[1],
    })
}

/// Which of the field's lines the input block starts at: the least it can
/// scroll and still show the cursor. Derived rather than stored, so the view
/// cannot drift out of step with the cursor.
pub(crate) fn pane_todo_edit_line_scroll(field: &TextField, rows: u16) -> usize {
    field
        .cursor_line()
        .saturating_sub(rows.max(1).saturating_sub(1) as usize)
}

/// Display columns the input block's text is shifted left by, so the cursor
/// stays visible on a line wider than the block. Applied to every row, not
/// just the cursor's, so the lines stay aligned with each other.
pub(crate) fn pane_todo_edit_column_scroll(field: &TextField, width: u16) -> usize {
    field
        .cursor_column()
        .saturating_sub(width.max(1).saturating_sub(1) as usize)
}

/// The part of the input block that holds the todo itself, once its padding is
/// taken out. Render and hit-test share it, so the cursor lands where the
/// pointer is.
pub(crate) fn pane_todo_edit_text_area(input: Rect) -> Rect {
    Rect::new(
        input.x + INPUT_PADDING,
        input.y,
        input.width.saturating_sub(INPUT_PADDING * 2),
        input.height,
    )
}

/// One rendered row of the input block: the visible slice of `line`, with the
/// character under the cursor picked out when the cursor is on this line.
fn input_row_line(
    line: &str,
    column_scroll: usize,
    width: usize,
    cursor_column: Option<usize>,
    text_style: Style,
    cursor_style: Style,
) -> Line<'static> {
    let (mut before, mut under, mut after) = (String::new(), String::new(), String::new());
    let mut column = 0usize;
    for ch in line.chars() {
        let start = column;
        column += crate::ui::text::char_display_width(ch);
        if start < column_scroll || start - column_scroll >= width {
            continue;
        }
        match cursor_column {
            Some(cursor) if start == cursor => under.push(ch),
            Some(cursor) if start < cursor => before.push(ch),
            _ => after.push(ch),
        }
    }
    // At the end of a line there is no character to sit on, so the cursor
    // takes a blank cell — which is also where a fresh todo starts.
    if cursor_column.is_some() && under.is_empty() {
        under.push(' ');
    }
    Line::from(vec![
        Span::styled(" ".repeat(INPUT_PADDING as usize), text_style),
        Span::styled(before, text_style),
        Span::styled(under, cursor_style),
        Span::styled(after, text_style),
    ])
}

pub(super) fn render_pane_todo_edit_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(edit) = app.pane_todo_edit() else {
        return;
    };
    super::dim_background(frame, area);

    // `todo/note` in both, matching the session board: what a pane records is
    // as often a note to self as a task. Both arms move together — naming the
    // thing a note while it is composed and a todo the moment you reopen it
    // would be worse than naming it neither.
    let title = if edit.todo_id.is_some() {
        "edit todo/note"
    } else {
        "new todo/note"
    };
    let Some(inner) = render_modal_shell(
        frame,
        area,
        PANE_TODO_EDIT_POPUP_WIDTH,
        PANE_TODO_EDIT_POPUP_HEIGHT,
        &app.palette,
    ) else {
        return;
    };
    let Some(rects) = pane_todo_edit_rects(inner) else {
        return;
    };

    render_modal_header(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        title,
        &app.palette,
    );

    frame.render_widget(Clear, rects.input);
    let input_style = Style::default()
        .fg(app.palette.text)
        .bg(app.palette.surface0);
    let cursor_style = Style::default()
        .fg(app.palette.surface0)
        .bg(app.palette.accent);
    let text_area = pane_todo_edit_text_area(rects.input);
    let line_scroll = pane_todo_edit_line_scroll(&edit.text, rects.input.height);
    let column_scroll = pane_todo_edit_column_scroll(&edit.text, text_area.width);
    let cursor_line = edit.text.cursor_line();
    let lines: Vec<&str> = edit.text.lines().collect();
    for row in 0..rects.input.height {
        let rect = Rect::new(rects.input.x, rects.input.y + row, rects.input.width, 1);
        let idx = line_scroll + row as usize;
        let line = lines.get(idx).copied().unwrap_or("");
        frame.render_widget(
            Paragraph::new(input_row_line(
                line,
                column_scroll,
                text_area.width as usize,
                (idx == cursor_line).then(|| edit.text.cursor_column()),
                input_style,
                cursor_style,
            ))
            .style(input_style),
            rect,
        );
    }

    let priority_label = match edit.priority {
        crate::terminal::todo::TodoPriority::High => "high",
        crate::terminal::todo::TodoPriority::Normal => "normal",
        crate::terminal::todo::TodoPriority::Low => "low",
    };
    let field = |name: &str, hint: &str, value: String, value_style: Style| {
        Line::from(vec![
            Span::styled(
                format!(" {name:<10}"),
                Style::default().fg(app.palette.overlay0),
            ),
            Span::styled(
                format!("{hint:<5}"),
                Style::default().fg(app.palette.overlay1),
            ),
            Span::styled(value, value_style),
        ])
    };
    frame.render_widget(
        Paragraph::new(field(
            "priority",
            "⇥",
            priority_label.to_string(),
            Style::default().fg(app.pane_todo_indicator_color(Some(edit.priority))),
        )),
        rects.priority,
    );
    let mut link = field(
        "link",
        "^l",
        app.pane_todo_edit_link_label(),
        Style::default().fg(app.palette.blue),
    );
    // The row shows an address; without this it gives no way to travel to it.
    // Offered only while the link resolves, since `ctrl+g` is inert otherwise.
    if app.pane_todo_edit_link_target().is_some() {
        link.spans.push(Span::styled(
            "   ^g go",
            Style::default().fg(app.palette.overlay1),
        ));
    }
    frame.render_widget(Paragraph::new(link), rects.link);

    // Composing a new todo has no done state to show, so the reserved row
    // stays blank rather than offering a control that cannot be saved.
    if edit.todo_id.is_some() {
        frame.render_widget(
            Paragraph::new(field(
                "done",
                "^t",
                if edit.done { "yes" } else { "no" }.to_string(),
                Style::default().fg(if edit.done {
                    app.palette.green
                } else {
                    app.palette.overlay1
                }),
            )),
            rects.done,
        );
    }

    // `^s`, not `↵`: Enter inserts a newline in this field.
    render_action_button(
        frame,
        rects.save,
        Some("^s"),
        "save",
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        rects.cancel,
        Some("esc"),
        "cancel",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0)
            .add_modifier(Modifier::BOLD),
    );
}

pub(super) fn render_rename_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    super::dim_background(frame, area);

    let title = match app.mode {
        Mode::RenameWorkspace if app.pending_workspace_create_cwd.is_some() => "new workspace",
        Mode::RenameWorkspace => "rename workspace",
        Mode::RenameTab if app.creating_new_tab => "new tab",
        Mode::RenameTab => "rename tab",
        Mode::RenamePane => "rename pane",
        _ => return,
    };

    let Some(inner) = render_modal_shell(frame, area, 56, 7, &app.palette) else {
        return;
    };
    if inner.height < 4 {
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas::<5>(inner);

    render_modal_header(frame, rows[0], title, &app.palette);

    let input_rect = Rect::new(rows[2].x, rows[2].y, rows[2].width, 1);
    render_name_input_field(app, frame, input_rect);

    let (save_rect, clear_rect, cancel_rect) = rename_button_rects(inner);

    render_action_button(
        frame,
        save_rect,
        Some("↵"),
        "save",
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        clear_rect,
        Some("^c"),
        "clear",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        cancel_rect,
        Some("esc"),
        "cancel",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0)
            .add_modifier(Modifier::BOLD),
    );
}

pub(crate) fn new_linked_worktree_inner_rect(area: Rect) -> Option<Rect> {
    centered_popup_rect(
        area,
        NEW_LINKED_WORKTREE_POPUP_WIDTH,
        NEW_LINKED_WORKTREE_POPUP_HEIGHT,
    )
    .map(|popup| {
        Rect::new(
            popup.x + 1,
            popup.y + 1,
            popup.width.saturating_sub(2),
            popup.height.saturating_sub(2),
        )
    })
}

pub(crate) fn new_linked_worktree_button_rects(inner: Rect) -> (Rect, Rect) {
    let rects = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("↵"),
                label: "create and open",
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: "cancel",
            },
        ],
        2,
        inner.height.saturating_sub(1),
    );
    (rects[0], rects[1])
}

pub(crate) fn remove_worktree_popup_rect(area: Rect) -> Option<Rect> {
    centered_popup_rect(area, 72, 11)
}

pub(crate) fn remove_worktree_button_rects(inner: Rect, force_confirmation: bool) -> (Rect, Rect) {
    let primary_label = if force_confirmation {
        "delete anyway"
    } else {
        "remove"
    };
    let rects = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("↵"),
                label: primary_label,
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: "cancel",
            },
        ],
        2,
        inner.height.saturating_sub(1),
    );
    (rects[0], rects[1])
}

pub(crate) fn open_existing_worktree_inner_rect(area: Rect, entry_count: usize) -> Option<Rect> {
    let height = (entry_count as u16)
        .saturating_mul(2)
        .saturating_add(8)
        .clamp(13, 27);
    centered_popup_rect(area, 96, height).map(|popup| {
        Rect::new(
            popup.x + 1,
            popup.y + 1,
            popup.width.saturating_sub(2),
            popup.height.saturating_sub(2),
        )
    })
}

pub(crate) fn open_existing_worktree_max_visible_rows(inner: Rect) -> usize {
    usize::from(inner.height.saturating_sub(6) / 2)
}

pub(crate) fn open_existing_worktree_visible_start(
    open: &WorktreeOpenState,
    max_rows: usize,
) -> usize {
    let filtered = open.filtered_indices();
    let selected = open.selected_entry_index().unwrap_or(open.selected);
    let selected_pos = filtered
        .iter()
        .position(|idx| *idx == selected)
        .unwrap_or(0);
    // The kit's nearest-edge reveal, from a standing start: this picker keeps
    // no scroll of its own, so its window is re-derived from the selection
    // every frame rather than remembered.
    crate::ui::overlay::reveal_scroll(0, selected_pos, max_rows, filtered.len())
}

pub(crate) fn open_existing_worktree_button_rects(inner: Rect) -> (Rect, Rect) {
    let rects = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("↵"),
                label: "open",
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: "cancel",
            },
        ],
        2,
        inner.height.saturating_sub(1),
    );
    (rects[0], rects[1])
}

/// Grows with the grouped list — space headings included — up to the same
/// ceiling the flat picker used, past which the list scrolls.
///
/// Seven rows of chrome sit around the list: the modal border, the header
/// block (its title, its subtitle and the blank row under them, per
/// [`crate::ui::widgets::HEADER_ROWS`]) and the footer block.
/// Rows the picker's header block occupies: its title, its subtitle, and the
/// blank row under them. One more than [`crate::ui::widgets::HEADER_ROWS`]
/// because this overlay's header is two lines rather than one.
pub(crate) const PANE_MOVE_TARGET_HEADER_ROWS: u16 = crate::ui::widgets::HEADER_ROWS + 1;

pub(crate) fn pane_move_target_height(item_count: usize) -> u16 {
    (item_count as u16).saturating_add(7).clamp(8, 20)
}

pub(crate) fn pane_move_target_inner_rect(area: Rect, item_count: usize) -> Option<Rect> {
    centered_popup_rect(area, 64, pane_move_target_height(item_count)).map(|popup| {
        Rect::new(
            popup.x + 1,
            popup.y + 1,
            popup.width.saturating_sub(2),
            popup.height.saturating_sub(2),
        )
    })
}

pub(crate) fn pane_move_target_button_rects(inner: Rect) -> (Rect, Rect) {
    let rects = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("↵"),
                label: "move",
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: "cancel",
            },
        ],
        2,
        footer_split(inner, true)
            .1
            .unwrap_or_else(|| inner.height.saturating_sub(1)),
    );
    (rects[0], rects[1])
}

/// Row text for a picked destination. Tabs keep the number/label shape the flat
/// picker used; the two creating destinations name what they create.
pub(crate) fn pane_move_target_row_label(entry: &crate::app::state::PaneMoveTargetEntry) -> String {
    match &entry.target {
        crate::app::state::PaneMoveTarget::Tab { .. } => {
            if entry.label.is_empty() {
                format!("tab {}", entry.number)
            } else {
                format!("{} · {}", entry.number, entry.label)
            }
        }
        crate::app::state::PaneMoveTarget::NewTab { .. } => "new tab".to_string(),
        crate::app::state::PaneMoveTarget::NewSpace => "new space".to_string(),
    }
}

pub(super) fn render_pane_move_target_picker_overlay(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
) {
    use crate::app::state::PaneMoveTargetItem;

    let Some(picker) = app.pane_move_target_picker() else {
        return;
    };

    super::dim_background(frame, area);
    let Some(inner) = render_modal_shell(
        frame,
        area,
        64,
        pane_move_target_height(picker.items.len()),
        &app.palette,
    ) else {
        return;
    };
    if inner.height < 6 {
        return;
    }

    render_modal_header(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        "move pane",
        &app.palette,
    );
    frame.render_widget(
        Paragraph::new(" select a destination").style(Style::default().fg(app.palette.overlay0)),
        Rect::new(inner.x, inner.y.saturating_add(1), inner.width, 1),
    );

    let (content, _) = footer_split(inner, true);
    // The title and its subtitle are one header block, and the blank row that
    // follows them belongs to it.
    let max_rows = usize::from(content.height.saturating_sub(PANE_MOVE_TARGET_HEADER_ROWS));
    let start = picker
        .list
        .selected
        .saturating_sub(max_rows.saturating_sub(1));
    for (visible_idx, item) in picker.items.iter().skip(start).take(max_rows).enumerate() {
        let item_idx = start + visible_idx;
        let row = Rect::new(
            inner.x,
            inner
                .y
                .saturating_add(PANE_MOVE_TARGET_HEADER_ROWS + visible_idx as u16),
            inner.width,
            1,
        );
        let (text, style) = match item {
            // Same weight the sidebar gives its section headings, so the group
            // reads as a heading and never as a destination.
            PaneMoveTargetItem::SpaceHeading { label } => (
                format!(" {label}"),
                Style::default()
                    .fg(app.palette.overlay0)
                    .add_modifier(Modifier::BOLD),
            ),
            PaneMoveTargetItem::Destination(entry) => {
                let selected = item_idx == picker.list.selected;
                let marker = if selected { "›" } else { " " };
                let style = if selected {
                    Style::default()
                        .fg(app.palette.text)
                        .bg(app.palette.surface0)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.palette.subtext0)
                };
                (
                    format!("{marker} {}", pane_move_target_row_label(entry)),
                    style,
                )
            }
        };
        frame.render_widget(
            Paragraph::new(truncate_end(&text, inner.width as usize)).style(style),
            row,
        );
    }

    let (move_rect, cancel_rect) = pane_move_target_button_rects(inner);
    render_action_button(
        frame,
        move_rect,
        Some("↵"),
        "move",
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        cancel_rect,
        Some("esc"),
        "cancel",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0)
            .add_modifier(Modifier::BOLD),
    );
}

pub(super) fn render_new_linked_worktree_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(create) = app.worktree_create() else {
        return;
    };

    super::dim_background(frame, area);
    let Some(inner) = render_modal_shell(
        frame,
        area,
        NEW_LINKED_WORKTREE_POPUP_WIDTH,
        NEW_LINKED_WORKTREE_POPUP_HEIGHT,
        &app.palette,
    ) else {
        return;
    };
    if inner.height < 9 {
        return;
    }

    // `rows[1]` is the blank row the header block reserves; see
    // `crate::ui::widgets::HEADER_ROWS`.
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas::<9>(inner);

    render_modal_header(frame, rows[0], "new worktree", &app.palette);

    frame.render_widget(
        Paragraph::new(" branch").style(Style::default().fg(app.palette.overlay0)),
        rows[2],
    );
    let input_rect = Rect::new(rows[3].x, rows[3].y, rows[3].width, 1);
    render_name_input_field(app, frame, input_rect);

    let checkout = create.checkout_path.display().to_string();
    frame.render_widget(
        Paragraph::new(" checkout").style(Style::default().fg(app.palette.overlay0)),
        rows[4],
    );
    frame.render_widget(
        Paragraph::new(format!(" {checkout}")).style(Style::default().fg(app.palette.subtext0)),
        rows[5],
    );

    if create.creating {
        frame.render_widget(
            Paragraph::new(" creating…").style(Style::default().fg(app.palette.overlay0)),
            rows[6],
        );
    } else if let Some(error) = &create.error {
        frame.render_widget(
            Paragraph::new(format!(" {error}"))
                .style(Style::default().fg(app.palette.red))
                .wrap(Wrap { trim: false }),
            rows[6],
        );
    }

    let (create_rect, cancel_rect) = new_linked_worktree_button_rects(inner);
    render_action_button(
        frame,
        create_rect,
        Some("↵"),
        "create and open",
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        cancel_rect,
        Some("esc"),
        "cancel",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0)
            .add_modifier(Modifier::BOLD),
    );
}

pub(super) fn render_remove_worktree_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(remove) = app.worktree_remove() else {
        return;
    };

    super::dim_background(frame, area);
    let Some(popup) = remove_worktree_popup_rect(area) else {
        return;
    };
    let Some(inner) = render_panel_shell(frame, popup, app.palette.red, app.palette.panel_bg)
    else {
        return;
    };

    // `rows[1]` is the blank row the header block reserves; see
    // `crate::ui::widgets::HEADER_ROWS`.
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas::<9>(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            " delete worktree checkout?",
            Style::default()
                .fg(app.palette.red)
                .add_modifier(Modifier::BOLD),
        )])),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(" This removes the checkout folder:")
            .style(Style::default().fg(app.palette.overlay0)),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(format!(" {}", remove.path.display()))
            .style(Style::default().fg(app.palette.text)),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(" The branch is not deleted. The Herdr workspace will close.")
            .style(Style::default().fg(app.palette.overlay0)),
        rows[3],
    );
    if remove.force_confirmation {
        frame.render_widget(
            Paragraph::new(" Dirty or untracked files will be permanently deleted.")
                .style(Style::default().fg(app.palette.red)),
            rows[4],
        );
    }
    if remove.removing {
        frame.render_widget(
            Paragraph::new(" removing…").style(Style::default().fg(app.palette.overlay0)),
            rows[5],
        );
    } else if let Some(error) = &remove.error {
        frame.render_widget(
            Paragraph::new(format!(" {error}")).style(Style::default().fg(app.palette.red)),
            rows[5],
        );
    }

    let (remove_rect, cancel_rect) = remove_worktree_button_rects(inner, remove.force_confirmation);
    let remove_label = if remove.force_confirmation {
        "delete anyway"
    } else {
        "remove"
    };
    render_action_button(
        frame,
        remove_rect,
        Some("↵"),
        remove_label,
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.red)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        cancel_rect,
        Some("esc"),
        "cancel",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0)
            .add_modifier(Modifier::BOLD),
    );
}

pub(super) fn render_open_existing_worktree_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(open) = app.worktree_open() else {
        return;
    };

    super::dim_background(frame, area);
    let height = (open.entries.len() as u16)
        .saturating_mul(2)
        .saturating_add(7)
        .clamp(12, 26);
    let Some(inner) = render_modal_shell(frame, area, 96, height, &app.palette) else {
        return;
    };
    if inner.height < 9 {
        return;
    }

    render_modal_header(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        "open worktree",
        &app.palette,
    );
    render_open_worktree_search(
        app,
        frame,
        Rect::new(inner.x, inner.y + HEADER_ROWS, inner.width, 1),
        open,
    );
    frame.render_widget(
        Paragraph::new("─".repeat(inner.width as usize))
            .style(Style::default().fg(app.palette.surface1)),
        Rect::new(
            inner.x,
            inner.y.saturating_add(HEADER_ROWS + 1),
            inner.width,
            1,
        ),
    );

    let filtered = open.filtered_indices();
    let max_rows = open_existing_worktree_max_visible_rows(inner);
    let start = open_existing_worktree_visible_start(open, max_rows);
    for (visible_idx, entry_idx) in filtered.iter().skip(start).take(max_rows).enumerate() {
        let Some(entry) = open.entries.get(*entry_idx) else {
            continue;
        };
        let selected = Some(*entry_idx) == open.selected_entry_index();
        let y = inner
            .y
            .saturating_add(HEADER_ROWS + 2 + (visible_idx as u16 * 2));
        let marker = if selected { "›" } else { " " };
        let row_style = if selected {
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.surface0)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.palette.subtext0)
        };
        let path_style = if selected {
            Style::default()
                .fg(app.palette.subtext0)
                .bg(app.palette.surface0)
        } else {
            Style::default().fg(app.palette.overlay0)
        };
        let status = entry.status_label();
        let title_width = inner
            .width
            .saturating_sub(display_width_u16(status))
            .saturating_sub(4) as usize;
        let mut title = format!(
            "{marker} {}",
            truncate_end(&entry.display_name(), title_width)
        );
        if !status.is_empty() {
            let pad = inner
                .width
                .saturating_sub(display_width_u16(&title))
                .saturating_sub(display_width_u16(status))
                .max(1);
            title.push_str(&" ".repeat(pad as usize));
            title.push_str(status);
        }
        frame.render_widget(
            Paragraph::new(truncate_end(&title, inner.width as usize)).style(row_style),
            Rect::new(inner.x, y, inner.width, 1),
        );
        frame.render_widget(
            Paragraph::new(truncate_end(
                &format!("  {}", entry.path.display()),
                inner.width as usize,
            ))
            .style(path_style),
            Rect::new(inner.x, y.saturating_add(1), inner.width, 1),
        );
    }

    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new(" no matching worktrees")
                .style(Style::default().fg(app.palette.overlay0)),
            Rect::new(inner.x, inner.y.saturating_add(3), inner.width, 1),
        );
    }

    if let Some(error) = &open.error {
        frame.render_widget(
            Paragraph::new(format!(" {error}")).style(Style::default().fg(app.palette.red)),
            Rect::new(
                inner.x,
                inner.y + inner.height.saturating_sub(2),
                inner.width,
                1,
            ),
        );
    }

    let (open_rect, cancel_rect) = open_existing_worktree_button_rects(inner);
    render_action_button(
        frame,
        open_rect,
        Some("↵"),
        "open",
        Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD),
    );
    render_action_button(
        frame,
        cancel_rect,
        Some("esc"),
        "cancel",
        Style::default()
            .fg(app.palette.text)
            .bg(app.palette.surface0)
            .add_modifier(Modifier::BOLD),
    );
}

fn render_open_worktree_search(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
    open: &WorktreeOpenState,
) {
    let focus_style = if open.search_focused {
        Style::default()
            .fg(app.palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.palette.overlay0)
    };
    let filtered_count = open.filtered_indices().len();
    let count = if open.query.trim().is_empty() {
        format!("{} checkouts", open.entries.len())
    } else {
        format!("{filtered_count}/{} checkouts", open.entries.len())
    };
    let mut spans = vec![Span::styled(" / ", focus_style)];
    if open.query.trim().is_empty() {
        spans.push(Span::styled(
            "filter worktrees",
            Style::default().fg(app.palette.overlay0),
        ));
    } else {
        spans.push(Span::styled(
            open.query.clone(),
            Style::default().fg(app.palette.text),
        ));
    }
    spans.push(Span::styled(
        format!(
            "{count:>width$}",
            width = area.width.saturating_sub(18) as usize
        ),
        Style::default().fg(app.palette.overlay0),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn confirm_close_overlay_text(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
) -> (String, String) {
    if let Some(pane_id) = app.confirm_respawn_pane {
        let label = app
            .pane_terminal(pane_id)
            .and_then(|terminal| terminal.border_label(true))
            .unwrap_or_else(|| "this pane".to_string());
        let outstanding = app
            .pane_terminal(pane_id)
            .map(|terminal| terminal.outstanding_todo_count())
            .unwrap_or(0);
        let detail = if outstanding == 1 {
            format!("{label} - restarts the process, 1 outstanding todo")
        } else if outstanding > 1 {
            format!("{label} - restarts the process, {outstanding} outstanding todos")
        } else {
            format!("{label} - restarts the process")
        };
        return ("Respawn pane and kill what is running?".to_string(), detail);
    }
    if let Some(pane_id) = app.confirm_close_pane {
        let outstanding = app
            .pane_terminal(pane_id)
            .map(|terminal| terminal.outstanding_todo_count())
            .unwrap_or(0);
        let label = app
            .pane_terminal(pane_id)
            .and_then(|terminal| terminal.border_label(true))
            .unwrap_or_else(|| "this pane".to_string());
        let todo_text = if outstanding == 1 {
            "1 outstanding todo".to_string()
        } else {
            format!("{outstanding} outstanding todos")
        };
        return (
            "Close pane with unfinished todos?".to_string(),
            format!("{label} - {todo_text}"),
        );
    }
    let ws_name = app
        .workspaces
        .get(app.selected)
        .map(|ws| ws.display_name_from(&app.terminals, terminal_runtimes))
        .unwrap_or_else(|| "?".to_string());
    let selected_space = app
        .workspaces
        .get(app.selected)
        .and_then(|ws| ws.worktree_space());
    let group_member_indices = selected_space
        .filter(|space| !space.is_linked_worktree)
        .map(|space| {
            app.workspaces
                .iter()
                .enumerate()
                .filter_map(|(idx, ws)| {
                    ws.worktree_space()
                        .is_some_and(|member| member.key == space.key)
                        .then_some(idx)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let closes_group = group_member_indices.len() > 1;
    let pane_count = if closes_group {
        group_member_indices
            .iter()
            .filter_map(|idx| app.workspaces.get(*idx))
            .map(|ws| ws.layout.pane_count())
            .sum()
    } else {
        app.workspaces
            .get(app.selected)
            .map(|ws| ws.layout.pane_count())
            .unwrap_or(0)
    };

    let pane_text = if pane_count == 1 {
        "1 pane".to_string()
    } else {
        format!("{pane_count} panes")
    };
    let workspace_text = if closes_group {
        let count = group_member_indices.len();
        if count == 1 {
            "1 workspace, ".to_string()
        } else {
            format!("{count} workspaces, ")
        }
    } else {
        String::new()
    };

    let title = if closes_group {
        "Close worktree group?"
    } else {
        "Close workspace?"
    };
    let detail = format!("{ws_name} — {workspace_text}{pane_text}");
    (title.to_string(), detail)
}

pub(super) fn render_confirm_close_overlay(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    frame: &mut Frame,
    area: Rect,
) {
    let (title, detail) = confirm_close_overlay_text(app, terminal_runtimes);

    super::dim_background(frame, area);

    let Some(popup) = confirm_close_popup_rect(area) else {
        return;
    };

    let warn = Style::default()
        .fg(app.palette.red)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(app.palette.overlay0);

    let title_line = Line::from(vec![Span::styled(format!(" {title}"), warn)]);

    let detail_line = Line::from(vec![
        Span::styled(
            format!(" {}", detail.split(" — ").next().unwrap_or(&detail)),
            Style::default()
                .fg(app.palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            detail
                .split_once(" — ")
                .map(|(_, rest)| format!(" — {rest}"))
                .unwrap_or_default(),
            dim,
        ),
    ]);

    let Some(inner) = render_panel_shell(frame, popup, app.palette.red, app.palette.panel_bg)
    else {
        return;
    };

    if inner.height >= 3 {
        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas::<4>(inner);

        frame.render_widget(Paragraph::new(title_line), rows[0]);
        frame.render_widget(Paragraph::new(detail_line), rows[1]);

        let (confirm_rect, cancel_rect) = confirm_close_button_rects(inner);
        render_action_button(
            frame,
            confirm_rect,
            Some("↵"),
            "confirm",
            Style::default()
                .fg(panel_contrast_fg(&app.palette))
                .bg(app.palette.red)
                .add_modifier(Modifier::BOLD),
        );
        render_action_button(
            frame,
            cancel_rect,
            Some("esc"),
            "cancel",
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.surface0)
                .add_modifier(Modifier::BOLD),
        );
    }
}

pub(crate) fn confirm_close_popup_rect(area: Rect) -> Option<Rect> {
    centered_popup_rect(area, 64, 6)
}

pub(crate) fn confirm_close_button_rects(inner: Rect) -> (Rect, Rect) {
    let rects = action_button_row_rects(
        inner,
        &[
            ActionButtonSpec {
                hint: Some("↵"),
                label: "confirm",
            },
            ActionButtonSpec {
                hint: Some("esc"),
                label: "cancel",
            },
        ],
        2,
        3,
    );
    (rects[0], rects[1])
}

#[cfg(test)]
mod tests {
    use crate::{
        app::{state::WorktreeCreateState, AppState, Mode},
        ui::text_field::TextField,
        ui::widgets::HEADER_ROWS,
        ui::PANE_MOVE_TARGET_HEADER_ROWS,
        workspace::Workspace,
    };
    use ratatui::{
        backend::TestBackend,
        buffer::Buffer,
        layout::{Position, Rect},
        Terminal,
    };

    use super::{
        confirm_close_overlay_text, display_width_u16, pane_move_target_inner_rect,
        pane_todo_edit_rects, pane_todo_edit_text_area, render_new_linked_worktree_overlay,
        render_pane_move_target_picker_overlay, render_pane_todo_edit_overlay,
        render_rename_overlay, Modifier, PANE_TODO_EDIT_POPUP_HEIGHT, PANE_TODO_EDIT_POPUP_WIDTH,
    };

    #[test]
    fn confirm_close_text_uses_live_workspace_cwd_label() {
        let mut app = AppState::test_new();
        let mut workspace = Workspace::test_new("initial");
        workspace.custom_name = None;
        workspace.identity_cwd = "/projects/original".into();
        let root_pane = workspace.tabs[0].root_pane;
        let terminal_id = workspace.tabs[0].panes[&root_pane]
            .attached_terminal_id
            .clone();
        app.workspaces = vec![workspace];
        app.ensure_test_terminals();
        app.terminals.get_mut(&terminal_id).unwrap().cwd = "/projects/current".into();
        app.selected = 0;

        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        let (title, detail) = confirm_close_overlay_text(&app, &terminal_runtimes);

        assert_eq!(title, "Close workspace?");
        assert_eq!(detail, "current — 1 pane");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn confirm_close_text_prefers_live_runtime_cwd_over_stale_terminal_cwd() {
        let root = std::env::temp_dir().join(format!(
            "herdr-confirm-close-runtime-cwd-{}",
            std::process::id()
        ));
        let stale_cwd = root.join("original");
        let live_cwd = root.join("current");
        std::fs::create_dir_all(&live_cwd).unwrap();

        let mut app = AppState::test_new();
        let mut workspace = Workspace::test_new("initial");
        workspace.custom_name = None;
        workspace.identity_cwd = stale_cwd.clone();
        let root_pane = workspace.tabs[0].root_pane;
        let terminal_id = workspace.tabs[0].panes[&root_pane]
            .attached_terminal_id
            .clone();
        app.workspaces = vec![workspace];
        app.ensure_test_terminals();
        app.selected = 0;

        let (events, _) = tokio::sync::mpsc::channel(4);
        let runtime = crate::terminal::TerminalRuntime::spawn(
            root_pane,
            24,
            80,
            live_cwd,
            0,
            crate::terminal_theme::TerminalTheme::default(),
            None,
            crate::pane::PaneShellConfig::new("/bin/sh", crate::config::ShellModeConfig::NonLogin),
            &crate::pane::PaneLaunchEnv::default(),
            events,
            std::sync::Arc::new(tokio::sync::Notify::new()),
            std::sync::Arc::new(crate::render_signal::RenderSignal::new()),
        )
        .unwrap();
        let mut terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        terminal_runtimes.insert(terminal_id, runtime);

        let (_, detail) = confirm_close_overlay_text(&app, &terminal_runtimes);

        assert_eq!(detail, "current — 1 pane");

        drop(terminal_runtimes);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirm_close_text_uses_selected_custom_name_instead_of_active_workspace_cwd() {
        let mut app = AppState::test_new();
        let active = Workspace::test_new("active");
        let selected = Workspace::test_new("selected");
        let selected_root = selected.tabs[0].root_pane;
        let selected_terminal_id = selected.tabs[0].panes[&selected_root]
            .attached_terminal_id
            .clone();
        app.workspaces = vec![active, selected];
        app.ensure_test_terminals();
        app.terminals.get_mut(&selected_terminal_id).unwrap().cwd = "/projects/current".into();
        app.active = Some(0);
        app.selected = 1;

        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        let (_, detail) = confirm_close_overlay_text(&app, &terminal_runtimes);

        assert_eq!(detail, "selected — 1 pane");
    }

    #[test]
    fn confirm_close_text_reports_parent_group_scope() {
        let mut app = AppState::test_new();
        let mut parent = Workspace::test_new("main");
        parent.worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: "/repo/herdr".into(),
            is_linked_worktree: false,
        });
        let mut child = Workspace::test_new("issue");
        child.worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: "/repo/herdr-issue".into(),
            is_linked_worktree: true,
        });
        app.workspaces = vec![parent, child];
        app.selected = 0;

        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        let (title, detail) = confirm_close_overlay_text(&app, &terminal_runtimes);

        assert_eq!(title, "Close worktree group?");
        assert_eq!(detail, "main — 2 workspaces, 2 panes");
    }

    #[test]
    fn new_worktree_error_renders_fatal_stderr_line() {
        let mut app = AppState::test_new();
        app.set_name_input("foo");
        app.set_overlay(crate::app::state::Overlay::NewLinkedWorktree(WorktreeCreateState {
            source_workspace_id: "source".into(),
            source_checkout_path: "/repo/herdr".into(),
            source_existing_membership: None,
            source_repo_root: "/repo/herdr".into(),
            repo_key: "repo-key".into(),
            repo_name: "herdr".into(),
            branch: "foo".into(),
            checkout_path: "/repo/.worktrees/herdr/foo".into(),
            error: Some(
                "Preparing worktree (new branch 'foo')\nfatal: a branch named 'foo' already exists"
                    .into(),
            ),
            creating: false,
        }));

        let mut terminal =
            Terminal::new(TestBackend::new(100, 30)).expect("test terminal should initialize");
        terminal
            .draw(|frame| render_new_linked_worktree_overlay(&app, frame, Rect::new(0, 0, 100, 30)))
            .expect("new worktree overlay should render");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("fatal: a branch named 'foo' already exists"));
    }

    #[test]
    fn new_worktree_hit_test_geometry_matches_modal_size() {
        let area = Rect::new(0, 0, 100, 30);
        let inner = super::new_linked_worktree_inner_rect(area).unwrap();
        let (create, cancel) = super::new_linked_worktree_button_rects(inner);

        assert_eq!(inner.width, super::NEW_LINKED_WORKTREE_POPUP_WIDTH - 2);
        assert_eq!(inner.height, super::NEW_LINKED_WORKTREE_POPUP_HEIGHT - 2);
        assert_eq!(create.y, inner.y + inner.height - 1);
        assert_eq!(cancel.y, inner.y + inner.height - 1);
    }

    const RENAME_AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    const WORKTREE_AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 30,
    };

    /// Reproduces the input row that `render_rename_overlay` lays out: the
    /// centred popup, the border inset, then the third row of the vertical
    /// split.
    fn rename_input_rect(area: Rect) -> Rect {
        let popup = super::centered_popup_rect(area, 56, 7).expect("popup fits");
        let inner = Rect::new(popup.x + 1, popup.y + 1, popup.width - 2, popup.height - 2);
        Rect::new(inner.x, inner.y + 2, inner.width, 1)
    }

    fn rename_overlay_caret_in(mode: Mode, name: &str) -> (Position, Buffer) {
        let mut app = AppState::test_new();
        app.mode = mode;
        app.set_name_input(name);

        let mut terminal = Terminal::new(TestBackend::new(RENAME_AREA.width, RENAME_AREA.height))
            .expect("test terminal");
        terminal
            .draw(|frame| render_rename_overlay(&app, frame, RENAME_AREA))
            .expect("rename overlay should render");
        let caret = terminal.get_cursor_position().expect("cursor position");
        (caret, terminal.backend().buffer().clone())
    }

    fn rename_overlay_caret(name: &str) -> Position {
        rename_overlay_caret_in(Mode::RenameWorkspace, name).0
    }

    fn worktree_overlay_caret(branch: &str) -> Position {
        let mut app = AppState::test_new();
        app.set_name_input(branch);
        app.set_overlay(crate::app::state::Overlay::NewLinkedWorktree(
            WorktreeCreateState {
                source_workspace_id: "source".into(),
                source_checkout_path: "/repo/herdr".into(),
                source_existing_membership: None,
                source_repo_root: "/repo/herdr".into(),
                repo_key: "repo-key".into(),
                repo_name: "herdr".into(),
                branch: branch.into(),
                checkout_path: "/repo/.worktrees/herdr/foo".into(),
                error: None,
                creating: false,
            },
        ));

        let mut terminal =
            Terminal::new(TestBackend::new(WORKTREE_AREA.width, WORKTREE_AREA.height))
                .expect("test terminal");
        terminal
            .draw(|frame| render_new_linked_worktree_overlay(&app, frame, WORKTREE_AREA))
            .expect("new worktree overlay should render");
        terminal.get_cursor_position().expect("cursor position")
    }

    #[test]
    fn rename_overlay_anchors_the_host_cursor_to_the_input_caret() {
        let input = rename_input_rect(RENAME_AREA);

        // Without an explicit cursor the frame carries none, the client parks the
        // host cursor where the focused pane last reported it, and the IME
        // composes there instead of in the dialog.
        assert_eq!(
            rename_overlay_caret(""),
            Position::new(input.x + 1, input.y),
            "empty input should put the caret past the one-column left padding"
        );
        assert_eq!(
            rename_overlay_caret("abcd"),
            Position::new(input.x + 5, input.y)
        );

        // The cell under the caret has to be blank: a host terminal draws its
        // cursor by inverting that cell, so a glyph there would swallow it.
        let (caret, buffer) = rename_overlay_caret_in(Mode::RenameWorkspace, "ab");
        assert_eq!(caret, Position::new(input.x + 3, input.y));
        assert_eq!(buffer[(caret.x, caret.y)].symbol(), " ");
        assert_eq!(buffer[(caret.x - 1, caret.y)].symbol(), "b");
    }

    #[test]
    fn rename_overlay_anchors_the_cursor_in_every_rename_mode() {
        let input = rename_input_rect(RENAME_AREA);
        let expected = Position::new(input.x + 3, input.y);

        for mode in [Mode::RenameWorkspace, Mode::RenameTab, Mode::RenamePane] {
            assert_eq!(
                rename_overlay_caret_in(mode, "ab").0,
                expected,
                "{mode:?} should anchor the caret like the other rename modes"
            );
        }
    }

    #[test]
    fn rename_overlay_caret_counts_wide_characters_as_two_columns() {
        let input = rename_input_rect(RENAME_AREA);

        // "あい" is two columns per character, so the caret sits two cells further
        // right than the two-column "ab".
        assert_eq!(
            rename_overlay_caret("あい"),
            Position::new(input.x + 5, input.y)
        );
        assert_eq!(
            rename_overlay_caret("aあ"),
            Position::new(input.x + 4, input.y)
        );
    }

    #[test]
    fn rename_overlay_caret_stays_inside_the_input_when_the_name_overflows() {
        let input = rename_input_rect(RENAME_AREA);
        let last_column = input.right() - 1;

        // The field is 54 columns wide. 51 characters is the last name whose
        // caret still lands strictly inside it; from 52 on the unclamped column
        // would leave the field and gets pinned to the final cell.
        assert_eq!(
            rename_overlay_caret(&"a".repeat(51)),
            Position::new(input.x + 52, input.y)
        );
        assert_eq!(
            rename_overlay_caret(&"a".repeat(53)),
            Position::new(last_column, input.y)
        );
        assert_eq!(
            rename_overlay_caret(&"a".repeat(200)),
            Position::new(last_column, input.y)
        );

        // The clamped cell has to stay blank as well, or the host cursor would
        // sit on a glyph and the IME would compose over it.
        let (caret, buffer) = rename_overlay_caret_in(Mode::RenameWorkspace, &"a".repeat(200));
        assert_eq!(caret, Position::new(last_column, input.y));
        assert_eq!(buffer[(caret.x, caret.y)].symbol(), " ");
        assert_eq!(buffer[(caret.x - 1, caret.y)].symbol(), "a");
    }

    #[test]
    fn rename_overlay_caret_reaches_the_frame_the_server_sends() {
        let input = rename_input_rect(RENAME_AREA);
        let mut app = AppState::test_new();
        app.mode = Mode::RenameWorkspace;
        app.set_name_input("ab");

        // The widget tests above stop at the ratatui frame. This one goes through
        // the server's cursor resolution, which is where the bug lived: the frame
        // used to leave here with `cursor: None`.
        let (_, cursor) =
            crate::server::render_stream::render_virtual(&mut app, RENAME_AREA, false);
        let cursor = cursor.expect("the modal caret should survive cursor resolution");

        assert_eq!((cursor.x, cursor.y), (input.x + 3, input.y));
        assert!(cursor.visible);
    }

    #[test]
    fn new_worktree_overlay_anchors_the_host_cursor_to_the_input_caret() {
        let popup = super::new_linked_worktree_inner_rect(WORKTREE_AREA).expect("popup fits");
        // Title, the header block's blank row, the "branch" label, then the input.
        let input = Rect::new(popup.x, popup.y + HEADER_ROWS + 1, popup.width, 1);

        assert_eq!(
            worktree_overlay_caret(""),
            Position::new(input.x + 1, input.y)
        );
        assert_eq!(
            worktree_overlay_caret("ab"),
            Position::new(input.x + 3, input.y)
        );
        assert_eq!(
            worktree_overlay_caret("あい"),
            Position::new(input.x + 5, input.y)
        );
    }

    #[test]
    fn pane_todo_edit_hit_test_geometry_matches_what_is_drawn() {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("todos")];
        app.active = Some(0);
        app.ensure_test_terminals();
        app.view.terminal_area = Rect::new(0, 0, 80, 24);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        app.open_new_pane_todo(pane_id);
        if let Some(edit) = app.pane_todo_edit_mut() {
            edit.text =
                TextField::from_text("rerun the deploy", crate::terminal::todo::MAX_TODO_TEXT_LEN);
        }

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| render_pane_todo_edit_overlay(&app, frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let inner = crate::ui::centered_popup_rect(
            Rect::new(0, 0, 80, 24),
            PANE_TODO_EDIT_POPUP_WIDTH,
            PANE_TODO_EDIT_POPUP_HEIGHT,
        )
        .map(|popup| Rect::new(popup.x + 1, popup.y + 1, popup.width - 2, popup.height - 2))
        .expect("popup should fit");
        let rects = pane_todo_edit_rects(inner).expect("edit rects should exist");

        let input: String = (rects.input.x..rects.input.x + rects.input.width)
            .map(|x| buffer[(x, rects.input.y)].symbol())
            .collect();
        assert!(input.contains("rerun the deploy"));
        // The cursor is a real insertion point now, drawn by inverting the
        // cell it sits on rather than by appending a glyph. Freshly opened, it
        // sits one column past the last character.
        let text_area = pane_todo_edit_text_area(rects.input);
        let cursor_x = text_area.x + display_width_u16("rerun the deploy");
        assert_eq!(
            buffer[(cursor_x, text_area.y)].style().bg,
            Some(app.palette.accent),
            "the cursor cell is picked out where the insertion point is"
        );

        let priority: String = (rects.priority.x..rects.priority.x + rects.priority.width)
            .map(|x| buffer[(x, rects.priority.y)].symbol())
            .collect();
        assert!(priority.contains("priority"));
        assert!(priority.contains("normal"));

        let link: String = (rects.link.x..rects.link.x + rects.link.width)
            .map(|x| buffer[(x, rects.link.y)].symbol())
            .collect();
        assert!(link.contains("link"));

        let save: String = (rects.save.x..rects.save.x + rects.save.width)
            .map(|x| buffer[(x, rects.save.y)].symbol())
            .collect();
        assert!(save.contains("save"));
    }

    /// The link row shows an address, so it has to offer a way to travel to
    /// it. `ctrl+l` re-picks the link and a click on the row does the same, so
    /// without the `^g` hint the row is a dead end that looks like a
    /// destination.
    #[test]
    fn the_link_row_advertises_go_only_while_the_link_resolves() {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("todos")];
        app.active = Some(0);
        app.ensure_test_terminals();
        app.view.terminal_area = Rect::new(0, 0, 80, 24);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let todo_id = app
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .add_todo(
                "go there",
                crate::terminal::todo::TodoPriority::Normal,
                None,
                100,
            )
            .expect("todo should be added")
            .id;

        let link_row = |app: &AppState| {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| render_pane_todo_edit_overlay(app, frame, frame.area()))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            let inner = crate::ui::centered_popup_rect(
                Rect::new(0, 0, 80, 24),
                PANE_TODO_EDIT_POPUP_WIDTH,
                PANE_TODO_EDIT_POPUP_HEIGHT,
            )
            .map(|popup| Rect::new(popup.x + 1, popup.y + 1, popup.width - 2, popup.height - 2))
            .expect("popup should fit");
            let rects = pane_todo_edit_rects(inner).expect("edit rects should exist");
            (rects.link.x..rects.link.x + rects.link.width)
                .map(|x| buffer[(x, rects.link.y)].symbol())
                .collect::<String>()
        };

        app.open_pane_todo_edit(pane_id, todo_id);
        let row = link_row(&app);
        assert!(row.contains("none"), "no link yet: {row}");
        assert!(!row.contains("^g"), "and so nowhere to go: {row}");

        // Staged through the picker, before any save: `ctrl+g` acts on the
        // staged choice, so the hint has to follow it rather than the store.
        if let Some(edit) = app.pane_todo_edit_mut() {
            edit.link = crate::app::state::PaneTodoEditLink::Set(pane_id);
        }
        let row = link_row(&app);
        assert!(
            row.contains("^g go"),
            "a staged live link can be followed: {row}"
        );

        if let Some(edit) = app.pane_todo_edit_mut() {
            edit.link = crate::app::state::PaneTodoEditLink::Clear;
        }
        assert!(!link_row(&app).contains("^g"), "a cleared link cannot");
    }

    /// A todo may hold more than one line, so the input block is several rows
    /// tall and scrolls to keep the insertion point visible rather than
    /// growing the modal to whatever was pasted in.
    #[test]
    fn the_input_block_shows_several_lines_and_scrolls_to_the_cursor() {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("todos")];
        app.active = Some(0);
        app.ensure_test_terminals();
        app.view.terminal_area = Rect::new(0, 0, 80, 24);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        app.open_new_pane_todo(pane_id);

        let inner = crate::ui::centered_popup_rect(
            Rect::new(0, 0, 80, 24),
            PANE_TODO_EDIT_POPUP_WIDTH,
            PANE_TODO_EDIT_POPUP_HEIGHT,
        )
        .map(|popup| Rect::new(popup.x + 1, popup.y + 1, popup.width - 2, popup.height - 2))
        .expect("popup should fit");
        let rects = pane_todo_edit_rects(inner).expect("edit rects should exist");
        assert_eq!(rects.input.height, super::PANE_TODO_EDIT_INPUT_ROWS);
        assert!(
            rects.input.y + rects.input.height <= rects.priority.y,
            "the block cannot overlap the field rows under it"
        );

        let render = |app: &AppState, row: u16| {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| render_pane_todo_edit_overlay(app, frame, frame.area()))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            (rects.input.x..rects.input.x + rects.input.width)
                .map(|x| buffer[(x, rects.input.y + row)].symbol())
                .collect::<String>()
        };

        // Three lines fill the block exactly.
        if let Some(edit) = app.pane_todo_edit_mut() {
            edit.text =
                TextField::from_text("one\ntwo\nthree", crate::terminal::todo::MAX_TODO_TEXT_LEN);
        }
        assert!(render(&app, 0).contains("one"));
        assert!(render(&app, 1).contains("two"));
        assert!(render(&app, 2).contains("three"));

        // A fourth scrolls the first out: the cursor lands on the last line,
        // and the block shows the last three.
        if let Some(edit) = app.pane_todo_edit_mut() {
            edit.text = TextField::from_text(
                "one\ntwo\nthree\nfour",
                crate::terminal::todo::MAX_TODO_TEXT_LEN,
            );
        }
        assert!(!render(&app, 0).contains("one"));
        assert!(render(&app, 0).contains("two"));
        assert!(render(&app, 2).contains("four"));
    }

    /// The title used to start in the frame's first inner column while every
    /// row under it started one column further in, so it read as stuck to the
    /// border. It now shares the rows' column.
    #[test]
    fn the_modal_title_lines_up_with_the_rows_under_it() {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("todos")];
        app.active = Some(0);
        app.ensure_test_terminals();
        app.view.terminal_area = Rect::new(0, 0, 80, 24);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        app.open_new_pane_todo(pane_id);

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| render_pane_todo_edit_overlay(&app, frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let inner = crate::ui::centered_popup_rect(
            Rect::new(0, 0, 80, 24),
            PANE_TODO_EDIT_POPUP_WIDTH,
            PANE_TODO_EDIT_POPUP_HEIGHT,
        )
        .map(|popup| Rect::new(popup.x + 1, popup.y + 1, popup.width - 2, popup.height - 2))
        .expect("popup should fit");
        let rects = pane_todo_edit_rects(inner).expect("edit rects should exist");

        let column_of = |row: Rect, needle: &str| {
            let text: String = (row.x..row.x + row.width)
                .map(|x| buffer[(x, row.y)].symbol())
                .collect();
            row.x + text.find(needle).expect("row should hold its label") as u16
        };

        let title_x = column_of(Rect::new(inner.x, inner.y, inner.width, 1), "new todo/note");
        let priority_x = column_of(rects.priority, "priority");

        assert!(
            title_x > inner.x,
            "the title is held off the frame, not drawn against it"
        );
        assert_eq!(
            title_x, priority_x,
            "the title starts in the same column as the rows under it"
        );
    }

    /// The done row is only drawn when editing an existing todo, so the
    /// new-todo geometry test above cannot cover it. Same property: the cells
    /// that say "done" are the cells `pane_todo_edit_rects` hands the mouse.
    #[test]
    fn the_done_row_is_drawn_where_it_is_hit_tested_when_editing() {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("todos")];
        app.active = Some(0);
        app.ensure_test_terminals();
        app.view.terminal_area = Rect::new(0, 0, 80, 24);
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let todo_id = app
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .add_todo(
                "ship it",
                crate::terminal::todo::TodoPriority::Normal,
                None,
                100,
            )
            .expect("todo should be added")
            .id;
        app.open_pane_todo_edit(pane_id, todo_id);

        let render = |app: &AppState| {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| render_pane_todo_edit_overlay(app, frame, frame.area()))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            let inner = crate::ui::centered_popup_rect(
                Rect::new(0, 0, 80, 24),
                PANE_TODO_EDIT_POPUP_WIDTH,
                PANE_TODO_EDIT_POPUP_HEIGHT,
            )
            .map(|popup| Rect::new(popup.x + 1, popup.y + 1, popup.width - 2, popup.height - 2))
            .expect("popup should fit");
            let rects = pane_todo_edit_rects(inner).expect("edit rects should exist");
            let row: String = (rects.done.x..rects.done.x + rects.done.width)
                .map(|x| buffer[(x, rects.done.y)].symbol())
                .collect();
            row
        };

        let row = render(&app);
        assert!(row.contains("done"), "the done row is labelled: {row}");
        assert!(
            row.contains("^t"),
            "it advertises its shortcut, which is no longer ^d — that is \
             delete-forward in the text field now: {row}"
        );
        assert!(row.contains("no"), "and shows the current state: {row}");

        app.toggle_pane_todo_edit_done();
        let row = render(&app);
        assert!(row.contains("yes"), "toggling repaints the same row: {row}");
    }

    #[test]
    fn confirm_close_text_names_the_unfinished_todos() {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("current")];
        app.active = Some(0);
        app.selected = 0;
        app.ensure_test_terminals();
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let terminal = app
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist");
        for text in ["one", "two"] {
            terminal
                .add_todo(text, crate::terminal::todo::TodoPriority::Normal, None, 100)
                .expect("todo should be added");
        }
        app.confirm_close_pane = Some(pane_id);

        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        let (title, detail) = confirm_close_overlay_text(&app, &terminal_runtimes);

        assert_eq!(title, "Close pane with unfinished todos?");
        assert!(
            detail.contains("2 outstanding"),
            "detail should count the outstanding todos: {detail}"
        );
    }

    #[test]
    fn confirm_respawn_text_says_the_process_is_being_replaced() {
        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("current")];
        app.active = Some(0);
        app.selected = 0;
        app.ensure_test_terminals();
        let pane_id = app.workspaces[0].tabs[0].root_pane;
        app.confirm_respawn_pane = Some(pane_id);

        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        let (title, detail) = confirm_close_overlay_text(&app, &terminal_runtimes);

        assert_eq!(title, "Respawn pane and kill what is running?");
        assert!(
            detail.contains("restarts the process"),
            "detail should say the process is replaced: {detail}"
        );
        assert!(
            !detail.contains("outstanding"),
            "a pane with no todos should not mention them: {detail}"
        );
    }
    #[test]
    fn pane_move_picker_renders_space_headings_and_the_new_space_row_last() {
        use crate::app::state::{
            PaneMoveTarget, PaneMoveTargetEntry, PaneMoveTargetItem, PaneMoveTargetPickerState,
        };

        let mut app = AppState::test_new();
        app.workspaces = vec![Workspace::test_new("main"), Workspace::test_new("other")];
        app.active = Some(0);
        app.ensure_test_terminals();
        let own = app.workspaces[0].id.clone();
        let other = app.workspaces[1].id.clone();
        let items = vec![
            PaneMoveTargetItem::SpaceHeading {
                label: "main".into(),
            },
            PaneMoveTargetItem::Destination(PaneMoveTargetEntry {
                workspace_id: Some(own.clone()),
                number: 2,
                label: "logs".into(),
                target: PaneMoveTarget::Tab {
                    tab_id: format!("{own}:t2"),
                },
            }),
            PaneMoveTargetItem::SpaceHeading {
                label: "other".into(),
            },
            PaneMoveTargetItem::Destination(PaneMoveTargetEntry {
                workspace_id: Some(other.clone()),
                number: 1,
                label: "shell".into(),
                target: PaneMoveTarget::Tab {
                    tab_id: format!("{other}:t1"),
                },
            }),
            PaneMoveTargetItem::Destination(PaneMoveTargetEntry {
                workspace_id: None,
                number: 0,
                label: String::new(),
                target: PaneMoveTarget::NewSpace,
            }),
        ];
        let item_count = items.len();
        app.open_overlay(crate::app::state::Overlay::PaneMoveTargetPicker(
            PaneMoveTargetPickerState::new("pane".into(), items),
        ));

        let area = Rect::new(0, 0, 80, 24);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| render_pane_move_target_picker_overlay(&app, frame, area))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let inner = pane_move_target_inner_rect(area, item_count).expect("picker rect");

        // Rows start below the header block: the title, its subtitle, and the
        // blank row under them.
        let row_text = |row: u16| -> String {
            (inner.x..inner.x + inner.width)
                .map(|x| buffer[(x, inner.y + PANE_MOVE_TARGET_HEADER_ROWS + row)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        };
        let row_style =
            |row: u16| buffer[(inner.x + 1, inner.y + PANE_MOVE_TARGET_HEADER_ROWS + row)].style();

        assert_eq!(row_text(0), " main");
        assert_eq!(row_style(0).fg, Some(app.palette.overlay0));
        assert!(row_style(0).add_modifier.contains(Modifier::BOLD));
        assert_ne!(row_style(0).bg, Some(app.palette.surface0));

        // The first destination, not the heading above it, carries the marker.
        assert_eq!(row_text(1), "› 2 · logs");
        assert_eq!(row_style(1).bg, Some(app.palette.surface0));

        // A destination in another space is shown under that space's heading.
        assert_eq!(row_text(2), " other");
        assert_eq!(row_text(3), "  1 · shell");

        assert_eq!(row_text(4), "  new space");
        assert_eq!(item_count, 5, "the new-space row is the last item");
    }
}
