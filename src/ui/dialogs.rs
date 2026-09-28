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
    action_button_row_rects, centered_popup_rect, panel_contrast_fg, render_action_button,
    render_modal_header, render_modal_shell, render_panel_shell, ActionButtonSpec, HEADER_ROWS,
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

/// Sized for reading: at 84 columns the text block holds ~80, so even a todo
/// at the store's 500-character cap wraps to about seven rows and is mostly
/// visible at once in the eight-row block.
pub(crate) const PANE_TODO_EDIT_POPUP_WIDTH: u16 = 84;
pub(crate) const PANE_TODO_EDIT_POPUP_HEIGHT: u16 = 20;

/// How many wrapped rows of a todo the modal shows at once. A todo is a note,
/// not a document, so the block is bounded and scrolls rather than growing
/// the modal to fit whatever was pasted into it.
pub(crate) const PANE_TODO_EDIT_INPUT_ROWS: u16 = 8;

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

/// Which wrapped visual row the input block starts at: the least it can
/// scroll and still show the cursor. Derived rather than stored, so the view
/// cannot drift out of step with the cursor. There is no horizontal
/// counterpart — wrapping is what removed it.
pub(crate) fn pane_todo_edit_row_scroll(
    rows: &[crate::ui::text_wrap::WrappedRow],
    field: &TextField,
    visible_rows: u16,
) -> usize {
    let (caret_row, _) = crate::ui::text_wrap::caret_visual_position(
        rows,
        field.cursor_line(),
        field.cursor_column(),
    );
    caret_row.saturating_sub(visible_rows.max(1).saturating_sub(1) as usize)
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

/// One rendered row of the input block: a wrapped slice of one logical line,
/// with the character under the cursor picked out when the cursor is on this
/// visual row.
fn input_row_line(
    slice: &str,
    cursor_column: Option<usize>,
    text_style: Style,
    cursor_style: Style,
) -> Line<'static> {
    let (mut before, mut under, mut after) = (String::new(), String::new(), String::new());
    let mut column = 0usize;
    for ch in slice.chars() {
        let start = column;
        column += crate::ui::text::char_display_width(ch);
        match cursor_column {
            Some(cursor) if start == cursor => under.push(ch),
            Some(cursor) if start < cursor => before.push(ch),
            _ => after.push(ch),
        }
    }
    // At the end of a row there is no character to sit on, so the cursor
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
    // Editing names the id — the same id a row shows and the CLI accepts —
    // so what the modal changes is what was just read off the board. A new
    // todo has no id until the store assigns one.
    let title = match edit.todo_id {
        Some(id) => format!("edit todo/note #{id}"),
        None => "new todo/note".to_string(),
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
        &title,
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
    let wrapped = crate::ui::text_wrap::wrap_layout(edit.text.text(), text_area.width as usize);
    let row_scroll = pane_todo_edit_row_scroll(&wrapped, &edit.text, rects.input.height);
    let (caret_row, caret_col) = crate::ui::text_wrap::caret_visual_position(
        &wrapped,
        edit.text.cursor_line(),
        edit.text.cursor_column(),
    );
    let lines: Vec<&str> = edit.text.lines().collect();
    for row in 0..rects.input.height {
        let rect = Rect::new(rects.input.x, rects.input.y + row, rects.input.width, 1);
        let idx = row_scroll + row as usize;
        let slice = wrapped
            .get(idx)
            .map(|wrapped_row| &lines[wrapped_row.line][wrapped_row.start..wrapped_row.end])
            .unwrap_or("");
        frame.render_widget(
            Paragraph::new(input_row_line(
                slice,
                (idx == caret_row).then_some(caret_col),
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

/// Rows the picker's header block occupies: its title, its search row, and
/// the blank row under them. One more than [`crate::ui::widgets::HEADER_ROWS`]
/// because this overlay's header is two lines rather than one.
pub(crate) const PANE_MOVE_TARGET_HEADER_ROWS: u16 = crate::ui::widgets::HEADER_ROWS + 1;

/// The picker's narrowest: a short session's names still leave its search row
/// and its buttons room to read.
pub(crate) const PANE_MOVE_TARGET_MIN_WIDTH: u16 = 48;

/// The picker's footer. Both boxes always survive: moving is the point of it,
/// and cancel is the way out.
pub(crate) fn pane_move_target_button_specs(
) -> [crate::ui::overlay::ButtonSpec<crate::app::state::PaneMoveTargetPickerButton>; 2] {
    use crate::app::state::PaneMoveTargetPickerButton;
    [
        crate::ui::overlay::ButtonSpec {
            button: PaneMoveTargetPickerButton::Move,
            hint: Some("↵"),
            label: "move",
            drop_rank: None,
        },
        crate::ui::overlay::ButtonSpec {
            button: PaneMoveTargetPickerButton::Cancel,
            hint: Some("esc"),
            label: "cancel",
            drop_rank: None,
        },
    ]
}

/// The widest the picker's status column may measure, before its padding: a
/// tab with many panes lists as many names as fit, then `+N`.
pub(crate) const PANE_MOVE_TARGET_STATUS_MAX_COLUMNS: u16 = 36;

/// What a row shows on the right: a tab's pane names, a space's activity, or
/// the reminder on the pane's own tab.
fn pane_move_target_status(item: &crate::app::state::PaneMoveTargetItem) -> String {
    use crate::app::state::PaneMoveTargetItem;
    match item {
        PaneMoveTargetItem::SpaceHeading { activity, .. } => activity.clone(),
        PaneMoveTargetItem::Here(_) => "you are here".to_string(),
        PaneMoveTargetItem::Destination(entry) => entry.facts.pane_names.join(", "),
        PaneMoveTargetItem::Gap => String::new(),
    }
}

/// A heading's text: the space and its pane count, as the navigator writes it.
fn pane_move_heading_text(label: &str, pane_count: usize) -> String {
    format!("{label} ({pane_count})")
}

/// The width of a row's left side: gutter, branch, icon and text, with a spare
/// column. The same parts [`render_pane_move_target_row`] draws.
fn pane_move_target_left_width(item: &crate::app::state::PaneMoveTargetItem) -> u16 {
    use crate::app::state::PaneMoveTargetItem;
    let text = match item {
        PaneMoveTargetItem::SpaceHeading {
            label, pane_count, ..
        } => pane_move_heading_text(label, *pane_count),
        PaneMoveTargetItem::Here(entry) | PaneMoveTargetItem::Destination(entry) => {
            pane_move_target_row_label(entry)
        }
        PaneMoveTargetItem::Gap => return 0,
    };
    let branch = if item.is_branch() { 4 } else { 0 };
    3 + branch + 2 + display_width_u16(&text) + 1
}

/// The width the picker's rows want, border included: its widest tree row plus
/// a status column as wide as its widest status (capped), and never less than
/// its own buttons. Measured from every destination, not the filtered ones, so
/// a query does not resize it.
pub(crate) fn pane_move_target_content_width(
    items: &[crate::app::state::PaneMoveTargetItem],
) -> u16 {
    let left = items
        .iter()
        .map(pane_move_target_left_width)
        .max()
        .unwrap_or(0);
    let status = pane_move_target_status_width(items);
    left.saturating_add(status).saturating_add(2).max(
        crate::ui::overlay::ButtonRow::natural_width(&pane_move_target_button_specs())
            .saturating_add(2),
    )
}

/// The status column, a space before it and a spare after; 0 when no row has
/// one.
fn pane_move_target_status_width(items: &[crate::app::state::PaneMoveTargetItem]) -> u16 {
    let widest = items
        .iter()
        .map(|item| display_width_u16(&pane_move_target_status(item)))
        .max()
        .unwrap_or(0);
    if widest == 0 {
        0
    } else {
        widest.min(PANE_MOVE_TARGET_STATUS_MAX_COLUMNS) + 2
    }
}

/// Row text for a destination: `tab 1`, `tab 1 · cc`, or the thing a creating
/// destination makes.
pub(crate) fn pane_move_target_row_label(entry: &crate::app::state::PaneMoveTargetEntry) -> String {
    match &entry.target {
        crate::app::state::PaneMoveTarget::Tab { .. } => {
            if entry.label.is_empty() {
                format!("tab {}", entry.number)
            } else {
                format!("tab {} · {}", entry.number, entry.label)
            }
        }
        crate::app::state::PaneMoveTarget::NewTab { .. } => "new tab".to_string(),
        crate::app::state::PaneMoveTarget::NewSpace => "new space".to_string(),
    }
}

/// Pane names that fit `budget` columns: all of them, or as many as fit
/// followed by `+N` for the rest.
pub(crate) fn fit_pane_names(names: &[String], budget: usize) -> String {
    let all = names.join(", ");
    if crate::ui::text::display_width(&all) <= budget {
        return all;
    }
    for shown in (1..names.len()).rev() {
        let text = format!("{}, +{}", names[..shown].join(", "), names.len() - shown);
        if crate::ui::text::display_width(&text) <= budget {
            return text;
        }
    }
    truncate_end(&all, budget)
}

/// The line under the list saying what the selected row would do.
fn pane_move_target_detail(picker: &crate::app::state::PaneMoveTargetPickerState) -> String {
    use crate::app::state::PaneMoveTarget;
    let Some(entry) = picker.selected_destination() else {
        return String::new();
    };
    match &entry.target {
        PaneMoveTarget::Tab { .. } => format!(
            "{} › {}: {}",
            entry.facts.space,
            pane_move_target_row_label(entry),
            entry.facts.pane_names.join(", ")
        ),
        PaneMoveTarget::NewTab { .. } => format!("a new tab in {}", entry.facts.space),
        PaneMoveTarget::NewSpace => {
            if picker.source_label.is_empty() {
                "a new space".to_string()
            } else {
                format!("a new space holding {}", picker.source_label)
            }
        }
    }
}

pub(super) fn render_pane_move_target_picker_overlay(
    app: &AppState,
    frame: &mut Frame,
    area: Rect,
) {
    use crate::app::state::{PaneMoveTargetItem, PaneMoveTargetPickerButton};

    let Some(picker) = app.pane_move_target_picker() else {
        return;
    };
    // The resolved geometry the mouse hit-tests against, so what is drawn and
    // what is clickable cannot diverge.
    let Some(geometry) = app.pane_move_target_picker_geometry() else {
        return;
    };
    let p = &app.palette;
    super::dim_background(frame, area);
    if render_panel_shell(frame, geometry.outer, p.accent, p.panel_bg).is_none() {
        return;
    }
    let header = geometry.header;
    if header.height >= 2 {
        render_pane_move_target_title(
            frame,
            Rect::new(header.x, header.y, header.width, 1),
            picker,
            p,
        );
        let destinations = picker
            .all_items
            .iter()
            .filter(|item| matches!(item, PaneMoveTargetItem::Destination(_)))
            .count();
        crate::ui::overlay::render_search_row(
            frame,
            Rect::new(header.x, header.y + 1, header.width, 1),
            &picker.search,
            crate::ui::overlay::SearchRow {
                placeholder: "search destinations",
                count: format!(
                    "{destinations} {}",
                    if destinations == 1 {
                        "destination"
                    } else {
                        "destinations"
                    }
                ),
                chip: None,
            },
            p,
        );
    }
    if header.height >= 3 {
        render_rule(frame, Rect::new(header.x, header.y + 2, header.width, 1), p);
    }

    let list = geometry.list;
    if picker.items.is_empty() && list.height > 0 {
        frame.render_widget(
            Paragraph::new(" no match").style(Style::default().fg(p.overlay0)),
            Rect::new(list.x, list.y, list.width, 1),
        );
    }
    let status_width = pane_move_target_status_draw_width(&picker.all_items, list.width);
    let (start, visible) = picker.list.window(list, picker.items.len());
    for (visible_idx, item) in picker.items.iter().skip(start).take(visible).enumerate() {
        let item_idx = start + visible_idx;
        let row = Rect::new(list.x, list.y + visible_idx as u16, list.width, 1);
        let last_branch = !picker
            .items
            .get(item_idx + 1)
            .is_some_and(PaneMoveTargetItem::is_branch);
        render_pane_move_target_row(
            app,
            frame,
            row,
            item,
            PaneMoveRowLook {
                selected: item_idx == picker.list.selected,
                last_branch,
                status_width,
            },
        );
    }
    render_pane_move_target_scrollbar(app, picker, frame, list);

    if let Some(detail) = geometry.detail {
        render_rule(frame, Rect::new(detail.x, detail.y, detail.width, 1), p);
        let text = crate::ui::text::middle_elide(
            &pane_move_target_detail(picker),
            detail.width.saturating_sub(2) as usize,
        );
        frame.render_widget(
            Paragraph::new(format!(" {text}")).style(Style::default().fg(p.overlay0)),
            Rect::new(detail.x, detail.y + 1, detail.width, 1),
        );
    }

    if let Some(buttons) = app.pane_move_target_picker_buttons() {
        for placed in buttons.placed() {
            let style = match placed.button {
                PaneMoveTargetPickerButton::Move => Style::default()
                    .fg(panel_contrast_fg(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD),
                PaneMoveTargetPickerButton::Cancel => Style::default()
                    .fg(p.text)
                    .bg(p.surface0)
                    .add_modifier(Modifier::BOLD),
            };
            render_action_button(frame, placed.rect, placed.hint, placed.label, style);
        }
    }
}

/// The status column a list of `list_width` draws for `items`: the measured
/// one, unless the list is narrower than labels and status need. The label
/// floor protects labels only as far as they measured.
pub(crate) fn pane_move_target_status_draw_width(
    items: &[crate::app::state::PaneMoveTargetItem],
    list_width: u16,
) -> u16 {
    let labels = items
        .iter()
        .map(pane_move_target_left_width)
        .max()
        .unwrap_or(0);
    pane_move_target_status_width(items)
        .min(list_width.saturating_sub(PANE_MOVE_TARGET_LABEL_FLOOR.min(labels)))
}

/// What the label side of a row keeps before its status column is cut: the
/// gutter, a branch, the icon and 12 columns of text.
const PANE_MOVE_TARGET_LABEL_FLOOR: u16 = 3 + 4 + 2 + 12;

/// Rows the picker keeps under its list for the detail: a rule and one line.
pub(crate) const PANE_MOVE_TARGET_DETAIL_ROWS: u16 = 2;

fn render_rule(frame: &mut Frame, area: Rect, p: &crate::app::state::Palette) {
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(p.surface1)),
        area,
    );
}

/// `move pane  <pane>  from <space> › <tab>`: what moves, and from where.
fn render_pane_move_target_title(
    frame: &mut Frame,
    area: Rect,
    picker: &crate::app::state::PaneMoveTargetPickerState,
    p: &crate::app::state::Palette,
) {
    let area = Rect::new(
        area.x.saturating_add(1),
        area.y,
        area.width.saturating_sub(2),
        1,
    );
    let mut spans = vec![Span::styled(
        "move pane",
        Style::default().fg(p.text).add_modifier(Modifier::BOLD),
    )];
    if !picker.source_label.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            picker.source_label.clone(),
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
        ));
    }
    if !picker.source_place.is_empty() {
        spans.push(Span::styled(
            format!("  from {}", picker.source_place),
            Style::default().fg(p.overlay0),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[derive(Debug, Clone, Copy)]
struct PaneMoveRowLook {
    selected: bool,
    /// Whether this branch is the last one drawn under its heading, so it
    /// takes `└──`. Worked out over the drawn rows, so a filter keeps the tree
    /// closed.
    last_branch: bool,
    status_width: u16,
}

/// One row of the tree, in the navigator's language: accent headings, surface
/// branches, the tab's number set apart from the word `tab`, and a status
/// column on the right.
fn render_pane_move_target_row(
    app: &AppState,
    frame: &mut Frame,
    rect: Rect,
    item: &crate::app::state::PaneMoveTargetItem,
    look: PaneMoveRowLook,
) {
    use crate::app::state::{PaneMoveTarget, PaneMoveTargetItem};

    let p = &app.palette;
    if matches!(item, PaneMoveTargetItem::Gap) {
        return;
    }
    let bar = Style::default()
        .bg(p.accent)
        .fg(panel_contrast_fg(p))
        .add_modifier(Modifier::BOLD);
    let pick = |style: Style| {
        if look.selected {
            bar
        } else {
            style.bg(p.panel_bg)
        }
    };
    let dim = Style::default().fg(p.overlay0);
    let symbols = app.state_icon_symbols();
    let colors = app.state_icon_colors();
    let here = matches!(item, PaneMoveTargetItem::Here(_));

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(
        if here { " ◆ " } else { "   " },
        pick(Style::default().fg(p.accent)),
    ));
    if item.is_branch() {
        spans.push(Span::styled(
            if look.last_branch {
                "└── "
            } else {
                "├── "
            },
            pick(Style::default().fg(p.surface1)),
        ));
    }
    let status_style;
    match item {
        PaneMoveTargetItem::SpaceHeading {
            label,
            pane_count,
            status,
            seen,
            ..
        } => {
            let (icon, icon_style) = super::status::state_icon(*status, *seen, &symbols, &colors);
            spans.push(Span::styled(icon, pick(icon_style)));
            spans.push(Span::styled(" ", pick(Style::default())));
            spans.push(Span::styled(
                pane_move_heading_text(label, *pane_count),
                pick(Style::default().fg(p.accent).add_modifier(Modifier::BOLD)),
            ));
            status_style = pick(dim);
        }
        PaneMoveTargetItem::Here(entry) => {
            spans.push(Span::styled(
                super::status::state_icon(entry.facts.status, entry.facts.seen, &symbols, &colors)
                    .0,
                pick(dim),
            ));
            spans.push(Span::styled(" ", pick(dim)));
            spans.push(Span::styled(pane_move_target_row_label(entry), pick(dim)));
            status_style = pick(dim.add_modifier(Modifier::ITALIC));
        }
        PaneMoveTargetItem::Destination(entry) => match &entry.target {
            PaneMoveTarget::Tab { .. } => {
                let (icon, icon_style) = super::status::state_icon(
                    entry.facts.status,
                    entry.facts.seen,
                    &symbols,
                    &colors,
                );
                spans.push(Span::styled(icon, pick(icon_style)));
                spans.push(Span::styled(" ", pick(Style::default())));
                spans.push(Span::styled("tab ", pick(dim)));
                spans.push(Span::styled(
                    entry.number.to_string(),
                    pick(Style::default().fg(p.text).add_modifier(Modifier::BOLD)),
                ));
                if !entry.label.is_empty() {
                    spans.push(Span::styled(" · ", pick(dim)));
                    spans.push(Span::styled(
                        entry.label.clone(),
                        pick(Style::default().fg(p.text)),
                    ));
                }
                status_style = pick(Style::default().fg(p.subtext0));
            }
            PaneMoveTarget::NewTab { .. } | PaneMoveTarget::NewSpace => {
                spans.push(Span::styled("+", pick(Style::default().fg(p.accent))));
                spans.push(Span::styled(" ", pick(Style::default())));
                spans.push(Span::styled(
                    pane_move_target_row_label(entry),
                    pick(Style::default().fg(p.overlay1)),
                ));
                status_style = pick(dim);
            }
        },
        PaneMoveTargetItem::Gap => return,
    }

    // Cut the left side to what the status column leaves it.
    let left_budget = rect.width.saturating_sub(look.status_width) as usize;
    let mut used = 0usize;
    let mut left = Vec::with_capacity(spans.len());
    for span in spans {
        let width = crate::ui::text::display_width(&span.content);
        if used + width <= left_budget {
            used += width;
            left.push(span);
        } else {
            let room = left_budget.saturating_sub(used);
            if room > 0 {
                left.push(Span::styled(truncate_end(&span.content, room), span.style));
            }
            break;
        }
    }
    frame.render_widget(
        Paragraph::new(Line::from(left)).style(pick(Style::default())),
        rect,
    );

    if look.status_width > 0 {
        let status = match item {
            PaneMoveTargetItem::Destination(entry) => fit_pane_names(
                &entry.facts.pane_names,
                look.status_width.saturating_sub(2) as usize,
            ),
            _ => truncate_end(
                &pane_move_target_status(item),
                look.status_width.saturating_sub(2) as usize,
            ),
        };
        let status_rect = Rect::new(
            rect.x + rect.width.saturating_sub(look.status_width),
            rect.y,
            look.status_width,
            1,
        );
        frame.render_widget(
            Paragraph::new(format!(" {status}")).style(status_style),
            status_rect,
        );
    }
}

fn render_pane_move_target_scrollbar(
    app: &AppState,
    picker: &crate::app::state::PaneMoveTargetPickerState,
    frame: &mut Frame,
    list: Rect,
) {
    let len = picker.items.len();
    let viewport = list.height as usize;
    if list.width <= 1 || viewport == 0 || len <= viewport {
        return;
    }
    let (start, _) = picker.list.window(list, len);
    let metrics = crate::pane::ScrollMetrics {
        viewport_rows: viewport,
        offset_from_bottom: len.saturating_sub(viewport).saturating_sub(start),
        max_offset_from_bottom: len.saturating_sub(viewport),
    };
    if !super::scrollbar::should_show_scrollbar(metrics) {
        return;
    }
    super::scrollbar::render_scrollbar(
        frame,
        metrics,
        Rect::new(list.x + list.width - 1, list.y, 1, list.height),
        app.palette.surface_dim,
        app.palette.overlay0,
        "▕",
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
        workspace::Workspace,
    };
    use ratatui::{
        backend::TestBackend,
        buffer::Buffer,
        layout::{Position, Rect},
        Terminal,
    };

    use super::{
        confirm_close_overlay_text, display_width_u16, pane_todo_edit_rects,
        pane_todo_edit_text_area, render_new_linked_worktree_overlay,
        render_pane_todo_edit_overlay, render_rename_overlay, Modifier,
        PANE_TODO_EDIT_POPUP_HEIGHT, PANE_TODO_EDIT_POPUP_WIDTH,
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

        // Eight lines fill the block exactly.
        let eight = (1..=8).map(|i| format!("line{i}")).collect::<Vec<_>>();
        if let Some(edit) = app.pane_todo_edit_mut() {
            edit.text =
                TextField::from_text(&eight.join("\n"), crate::terminal::todo::MAX_TODO_TEXT_LEN);
        }
        assert!(render(&app, 0).contains("line1"));
        assert!(render(&app, 7).contains("line8"));

        // A ninth scrolls the first out: the cursor lands on the last line,
        // and the block shows the last eight.
        if let Some(edit) = app.pane_todo_edit_mut() {
            edit.text = TextField::from_text(
                &format!("{}\nline9", eight.join("\n")),
                crate::terminal::todo::MAX_TODO_TEXT_LEN,
            );
        }
        assert!(!render(&app, 0).contains("line1"));
        assert!(render(&app, 0).contains("line2"));
        assert!(render(&app, 7).contains("line9"));

        // Prose wider than the block wraps onto the next row at a word
        // boundary instead of sliding out of view to the right, and an
        // explicit newline still starts its own row after the wrapped ones.
        let wide = format!("{} tail\nsecond", "word ".repeat(20).trim_end());
        if let Some(edit) = app.pane_todo_edit_mut() {
            let mut field = TextField::from_text(&wide, crate::terminal::todo::MAX_TODO_TEXT_LEN);
            field.place_cursor(0, 0);
            edit.text = field;
        }
        let first = render(&app, 0);
        let second = render(&app, 1);
        assert!(first.contains("word"), "first row: {first:?}");
        assert!(
            first.trim_end().len() < rects.input.width as usize,
            "the row breaks at a word, not at the edge: {first:?}"
        );
        assert!(second.contains("tail"), "wrapped remainder: {second:?}");
        assert!(render(&app, 2).contains("second"), "hard newline survives");
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
    /// Name a tab's panes, in layout order, the way an agent names its pane.
    fn name_tab_panes(app: &mut AppState, ws_idx: usize, tab_idx: usize, names: &[&str]) {
        let pane_ids = app.workspaces[ws_idx].tabs[tab_idx].layout.pane_ids();
        for (pane_id, name) in pane_ids.into_iter().zip(names) {
            let terminal_id = app.workspaces[ws_idx]
                .pane_state(pane_id)
                .expect("pane")
                .attached_terminal_id
                .clone();
            let terminal = app.terminals.get_mut(&terminal_id).expect("terminal");
            terminal.agent_name = Some((*name).to_string());
            terminal.state = crate::detect::AgentState::Idle;
        }
    }

    /// ac's session in miniature: the moving pane `herdr-fixes` in herdr's
    /// tab `cc`, macOS with two tabs, CONTEXT with one, and `extra` more
    /// spaces for a list that has to scroll.
    fn move_picker_session(extra: usize) -> AppState {
        use ratatui::layout::Direction;
        let mut app = crate::ui::test_support::app_with_one_pane("herdr");
        app.workspaces[0].tabs[0].custom_name = Some("cc".into());
        app.workspaces[0].test_split(Direction::Horizontal);
        let mut macos = Workspace::test_new("macOS");
        macos.test_split(Direction::Horizontal);
        macos.test_split(Direction::Horizontal);
        macos.test_add_tab(None);
        macos.active_tab = 0;
        app.workspaces.push(macos);
        app.workspaces.push(Workspace::test_new("CONTEXT"));
        for n in 0..extra {
            app.workspaces
                .push(Workspace::test_new(&format!("space-{n}")));
        }
        app.ensure_test_terminals();
        name_tab_panes(&mut app, 0, 0, &["herdr-relay", "herdr-fixes"]);
        name_tab_panes(
            &mut app,
            1,
            0,
            &["macos-relay-2", "macos-relay-3", "vpn-morning-issues-2"],
        );
        name_tab_panes(&mut app, 1, 1, &["keyboard-shortcuts"]);
        name_tab_panes(&mut app, 2, 0, &["context-relay"]);
        // The moving pane is the second one in herdr's tab.
        let moving = app.workspaces[0].tabs[0].layout.pane_ids()[1];
        app.workspaces[0].tabs[0].layout.focus_pane(moving);
        crate::ui::test_support::layout(&mut app);
        app
    }

    fn open_move_picker(app: &mut AppState, query: Option<&str>) {
        let mut picker = crate::app::pane_move_target_picker_for_state(
            app,
            &crate::terminal::TerminalRuntimeRegistry::default(),
        )
        .expect("the picker opens");
        if let Some(query) = query {
            picker.search.focus();
            picker.search.query.insert_str(query);
            picker.refilter();
        }
        app.open_overlay(crate::app::state::Overlay::PaneMoveTargetPicker(picker));
    }

    fn move_picker_snapshot(
        extra: usize,
        query: Option<&str>,
    ) -> crate::ui::test_support::OverlaySnapshot {
        let base = move_picker_session(extra);
        let mut open = move_picker_session(extra);
        open_move_picker(&mut open, query);
        crate::ui::test_support::layout(&mut open);
        crate::ui::test_support::overlay_snapshot(&base, &open)
    }

    #[test]
    fn snapshot_move_picker() {
        move_picker_snapshot(0, None).assert(
            Rect::new(10, 1, 60, 23),
            &[
                "┌──────────────────────────────────────────────────────────┐",
                "│ move pane  herdr-fixes  from herdr › tab 1 · cc          │",
                "│ / search destinations                     7 destinations │",
                "│──────────────────────────────────────────────────────────│",
                "│   ○ herdr (2)                                            │",
                "│ ◆ ├── ○ tab 1 · cc  you are here                         │",
                "│   └── + new tab                                          │",
                "│                                                          │",
                "│   ○ macOS (4)                                            │",
                "│   ├── ○ tab 1       macos-relay-2, macos-relay-3, +1     │",
                "│   ├── ○ tab 2       keyboard-shortcuts                   │",
                "│   └── + new tab                                          │",
                "│                                                          │",
                "│   ○ CONTEXT (1)                                          │",
                "│   ├── ○ tab 1       context-relay                        │",
                "│   └── + new tab                                          │",
                "│                                                          │",
                "│   + new space                                            │",
                "│──────────────────────────────────────────────────────────│",
                "│ a new tab in herdr                                       │",
                "│                                                          │",
                "│                   ↵ move    esc cancel                   │",
                "└──────────────────────────────────────────────────────────┘",
            ],
        );
    }

    #[test]
    fn snapshot_move_picker_searching() {
        // The box keeps its size; macOS's heading and tabs remain, the first
        // of them selected, and its last tab closes the tree.
        move_picker_snapshot(0, Some("mac")).assert(
            Rect::new(10, 1, 60, 23),
            &[
                "┌──────────────────────────────────────────────────────────┐",
                "│ move pane  herdr-fixes  from herdr › tab 1 · cc          │",
                "│ / mac                                     7 destinations │",
                "│──────────────────────────────────────────────────────────│",
                "│   ○ macOS (4)                                            │",
                "│   ├── ○ tab 1       macos-relay-2, macos-relay-3, +1     │",
                "│   ├── ○ tab 2       keyboard-shortcuts                   │",
                "│   └── + new tab                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│                                                          │",
                "│──────────────────────────────────────────────────────────│",
                "│ macOS › tab 1: macos-relay-…elay-3, vpn-morning-issues-2 │",
                "│                                                          │",
                "│                   ↵ move    esc cancel                   │",
                "└──────────────────────────────────────────────────────────┘",
            ],
        );
    }

    #[test]
    fn snapshot_move_picker_small() {
        // More spaces than rows: the list scrolls and shows a scrollbar.
        move_picker_snapshot(4, None).assert(
            Rect::new(10, 0, 60, 25),
            &[
                "┌──────────────────────────────────────────────────────────┐",
                "│ move pane  herdr-fixes  from herdr › tab 1 · cc          │",
                "│ / search destinations                    15 destinations │",
                "│──────────────────────────────────────────────────────────│",
                "│   ○ herdr (2)                                           ▕│",
                "│ ◆ ├── ○ tab 1 · cc  you are here                        ▕│",
                "│   └── + new tab                                         ▕│",
                "│                                                         ▕│",
                "│   ○ macOS (4)                                           ▕│",
                "│   ├── ○ tab 1       macos-relay-2, macos-relay-3, +1    ▕│",
                "│   ├── ○ tab 2       keyboard-shortcuts                  ▕│",
                "│   └── + new tab                                         ▕│",
                "│                                                         ▕│",
                "│   ○ CONTEXT (1)                                         ▕│",
                "│   ├── ○ tab 1       context-relay                       ▕│",
                "│   └── + new tab                                         ▕│",
                "│                                                         ▕│",
                "│   · space-0 (1)                                         ▕│",
                "│   ├── · tab 1       pane 1                              ▕│",
                "│   └── + new tab                                         ▕│",
                "│──────────────────────────────────────────────────────────│",
                "│ a new tab in herdr                                       │",
                "│                                                          │",
                "│                   ↵ move    esc cancel                   │",
                "└──────────────────────────────────────────────────────────┘",
            ],
        );
    }

    /// The picker's list rows as drawn, trailing blanks dropped, with the
    /// buffer to check styles on.
    fn move_picker_rows(app: &AppState) -> (Vec<String>, ratatui::buffer::Buffer, Rect) {
        let buffer = crate::ui::test_support::draw_sized(
            app,
            crate::ui::test_support::SNAPSHOT_WIDTH,
            crate::ui::test_support::SNAPSHOT_HEIGHT,
        );
        let list = app
            .pane_move_target_picker_geometry()
            .expect("picker geometry")
            .list;
        (
            crate::ui::test_support::rect_rows(&buffer, list),
            buffer,
            list,
        )
    }

    /// #110: the picker reads like the navigator: accent space names with a
    /// count, the tab number apart from the word `tab`, actions marked `+`,
    /// and the navigator's accent selection bar.
    #[test]
    fn move_picker_rows_are_styled_like_the_navigator() {
        let mut app = move_picker_session(0);
        open_move_picker(&mut app, None);
        crate::ui::test_support::layout(&mut app);
        let (rows, buffer, list) = move_picker_rows(&app);
        let p = &app.palette;
        let cell = |row: usize, text: &str| {
            let col = rows[row].find(text).expect("text on the row");
            let col = rows[row][..col].chars().count() as u16;
            buffer[(list.x + col, list.y + row as u16)].style()
        };

        let macos = rows
            .iter()
            .position(|row| row.contains("macOS (4)"))
            .expect("macOS heading");
        assert_eq!(cell(macos, "macOS").fg, Some(p.accent));
        assert!(cell(macos, "macOS").add_modifier.contains(Modifier::BOLD));

        let tab = macos + 1;
        assert!(rows[tab].contains("├── "), "{rows:#?}");
        assert_eq!(cell(tab, "tab").fg, Some(p.overlay0));
        assert_eq!(cell(tab, "1").fg, Some(p.text));
        assert!(cell(tab, "1").add_modifier.contains(Modifier::BOLD));
        assert_eq!(cell(tab, "macos-relay-2").fg, Some(p.subtext0));

        let new_tab = rows[macos..]
            .iter()
            .position(|row| row.contains("new tab"))
            .expect("new tab")
            + macos;
        assert!(rows[new_tab].contains("└── + new tab"), "{rows:#?}");
        assert_eq!(cell(new_tab, "+").fg, Some(p.accent));
        assert_eq!(cell(new_tab, "new tab").fg, Some(p.overlay1));

        // The pane's own tab: greyed, marked, and not the selection.
        assert!(rows[1].starts_with(" ◆ ├── "), "{rows:#?}");
        assert!(rows[1].contains("tab 1 · cc"));
        assert!(rows[1].ends_with("you are here"));
        assert_eq!(cell(1, "tab 1").fg, Some(p.overlay0));

        // The first destination is selected, drawn with the accent bar.
        assert!(rows[2].contains("+ new tab"));
        assert_eq!(cell(2, "new tab").bg, Some(p.accent));
    }

    #[test]
    fn move_picker_last_visible_child_gets_the_corner_branch() {
        let mut app = move_picker_session(0);
        // `keyboard` keeps only macOS's second tab: it is the last branch
        // drawn under the heading, so it closes the tree.
        open_move_picker(&mut app, Some("keyboard"));
        crate::ui::test_support::layout(&mut app);
        let (rows, _, _) = move_picker_rows(&app);

        assert!(rows[0].contains("macOS (4)"), "{rows:#?}");
        assert!(
            rows[1].contains("└── ") && rows[1].contains("tab 2"),
            "{rows:#?}"
        );
    }

    #[test]
    fn move_picker_detail_line_names_the_selected_tabs_panes() {
        let mut app = move_picker_session(0);
        open_move_picker(&mut app, Some("macos-relay"));
        crate::ui::test_support::layout(&mut app);
        let detail = app
            .pane_move_target_picker_geometry()
            .and_then(|geometry| geometry.detail)
            .expect("a detail line");
        let buffer = crate::ui::test_support::draw_sized(
            &app,
            crate::ui::test_support::SNAPSHOT_WIDTH,
            crate::ui::test_support::SNAPSHOT_HEIGHT,
        );
        let line = crate::ui::test_support::row_text_trimmed(
            &buffer,
            Rect::new(detail.x, detail.y + 1, detail.width, 1),
        );

        // Every pane is named; a line wider than the box is cut in the
        // middle, so the space, the tab and the last pane stay readable.
        assert!(line.starts_with(" macOS › tab 1: macos-relay-"), "{line}");
        assert!(line.ends_with("vpn-morning-issues-2"), "{line}");
    }

    #[test]
    fn move_picker_width_is_measured_from_tree_rows() {
        use crate::app::state::{
            PaneMoveTabFacts, PaneMoveTarget, PaneMoveTargetEntry, PaneMoveTargetItem,
        };
        let tab = |names: &[&str]| {
            PaneMoveTargetItem::Destination(PaneMoveTargetEntry {
                workspace_id: Some("w1".into()),
                number: 1,
                label: String::new(),
                target: PaneMoveTarget::Tab {
                    tab_id: "w1:t1".into(),
                },
                facts: PaneMoveTabFacts {
                    pane_names: names.iter().map(|name| name.to_string()).collect(),
                    ..Default::default()
                },
            })
        };
        let short = vec![
            PaneMoveTargetItem::heading("a-space-with-a-long-name"),
            tab(&["one"]),
        ];
        // `├── ` branch, gutter, icon and `tab 1` are narrower than the
        // heading: the heading sets the left side.
        let left = 3 + 2 + display_width_u16("a-space-with-a-long-name (0)") + 1;
        assert_eq!(
            super::pane_move_target_content_width(&short),
            left + 3 + 2 + 2,
        );

        let long = vec![
            PaneMoveTargetItem::heading("a-space-with-a-long-name"),
            tab(&["x".repeat(30).as_str(), "y".repeat(30).as_str()]),
        ];
        assert_eq!(
            super::pane_move_target_content_width(&long),
            left + super::PANE_MOVE_TARGET_STATUS_MAX_COLUMNS + 2 + 2,
            "the status column stops at its cap"
        );
    }

    /// #116: the live proof showed `clipper-relay-3, +2` where
    /// `clipper-relay-3, clipper-astra-2, +1` fits: a label floor wider than
    /// the picker's labels cut the status column the box was sized for.
    #[test]
    fn move_picker_status_is_not_cut_for_a_label_floor_the_labels_do_not_need() {
        use crate::app::state::{
            PaneMoveTabFacts, PaneMoveTarget, PaneMoveTargetEntry, PaneMoveTargetItem,
        };
        let items = vec![
            PaneMoveTargetItem::heading("clipper"),
            PaneMoveTargetItem::Destination(PaneMoveTargetEntry {
                workspace_id: Some("w5".into()),
                number: 1,
                label: String::new(),
                target: PaneMoveTarget::Tab {
                    tab_id: "w5:t1".into(),
                },
                facts: PaneMoveTabFacts {
                    pane_names: ["clipper-relay-3", "clipper-astra-2", "pane 7", "x"]
                        .iter()
                        .map(|name| name.to_string())
                        .collect(),
                    ..Default::default()
                },
            }),
        ];
        let list_width = super::pane_move_target_content_width(&items) - 2;

        assert_eq!(
            super::pane_move_target_status_draw_width(&items, list_width),
            super::PANE_MOVE_TARGET_STATUS_MAX_COLUMNS + 2,
        );
    }

    #[test]
    fn fit_pane_names_ends_with_the_count_of_the_rest() {
        let names: Vec<String> = ["macos-relay-2", "macos-relay-3", "vpn-morning-issues-2"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(
            super::fit_pane_names(&names, 80),
            "macos-relay-2, macos-relay-3, vpn-morning-issues-2"
        );
        assert_eq!(
            super::fit_pane_names(&names, 34),
            "macos-relay-2, macos-relay-3, +1"
        );
        assert_eq!(super::fit_pane_names(&names, 20), "macos-relay-2, +2");
    }
}
