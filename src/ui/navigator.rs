use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use super::{
    overlay::{render_search_row, SearchRow},
    scrollbar::{render_scrollbar, should_show_scrollbar},
    status::{state_icon, state_label_color},
    text::{display_width_u16, middle_elide, truncate_end},
    widgets::{panel_contrast_fg, render_panel_shell},
};
use crate::app::state::{
    navigator_display_lines, AppState, NavigatorDisplayLine, NavigatorPurpose, NavigatorRow,
    NavigatorStateFilter, NavigatorTarget,
};
use crate::terminal::TerminalRuntimeRegistry;

pub(super) fn render_navigator_overlay(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    frame: &mut Frame,
) {
    let popup = app.navigator_popup_rect();
    let Some(inner) = render_panel_shell(frame, popup, app.palette.accent, app.palette.panel_bg)
    else {
        return;
    };

    let search = app.navigator_search_rect();
    let body = app.navigator_body_rect();
    let detail = app.navigator_detail_rect();
    let footer = app.navigator_footer_rect();
    render_search(app, frame, search);

    if body.height > 0 {
        let rows = app.navigator_rows_from(terminal_runtimes);
        let lines = navigator_display_lines(&rows);
        render_separator(frame, Rect::new(inner.x, search.y + 1, inner.width, 1), app);
        render_rows(app, &rows, &lines, frame, body);
        render_navigator_scrollbar(app, lines.len(), frame, body);
    }
    render_detail(app, terminal_runtimes, frame, detail);
    render_footer(app, frame, footer);
}

fn render_search(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(nav) = app.navigator() else {
        return;
    };

    let count = app
        .workspaces
        .iter()
        .flat_map(|workspace| workspace.tabs.iter())
        .map(|tab| tab.panes.len())
        .sum::<usize>();
    // A state filter is shown as its chip in the query's place.
    let chip = nav.state_filter.map(|filter| {
        let mut spans = Vec::new();
        let (state, seen, label) = match filter {
            NavigatorStateFilter::Blocked => (crate::detect::AgentState::Blocked, true, "blocked"),
            NavigatorStateFilter::Working => (crate::detect::AgentState::Working, true, "working"),
            NavigatorStateFilter::Idle => (crate::detect::AgentState::Idle, true, "idle"),
            NavigatorStateFilter::Done => (crate::detect::AgentState::Idle, false, "done"),
        };
        push_state_chip(&mut spans, state, seen, label, app);
        spans
    });
    render_search_row(
        frame,
        area,
        &nav.search,
        SearchRow {
            // The picker has no title bar to name it, so the placeholder and
            // the footer verb below are what say which job the overlay is
            // doing.
            placeholder: if nav.purpose == NavigatorPurpose::PaneTodoLink {
                "search panes to link"
            } else {
                "search panes"
            },
            count: format!("{count} {}", if count == 1 { "pane" } else { "panes" }),
            chip,
        },
        &app.palette,
    );
}

fn push_state_chip(
    spans: &mut Vec<Span<'static>>,
    state: crate::detect::AgentState,
    seen: bool,
    label: &'static str,
    app: &AppState,
) {
    let (icon, icon_style) = state_icon(
        state,
        seen,
        &app.state_icon_symbols(),
        &app.state_icon_colors(),
    );
    // The chip row is `'static`; one short allocation per navigator chip.
    spans.push(Span::styled(
        icon.to_string(),
        icon_style.add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        label,
        Style::default()
            .fg(state_label_color(state, seen, &app.state_icon_colors()))
            .add_modifier(Modifier::BOLD),
    ));
}

fn render_separator(frame: &mut Frame, area: Rect, app: &AppState) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let line = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(app.palette.surface1)),
        area,
    );
}

fn render_rows(
    app: &AppState,
    rows: &[NavigatorRow],
    lines: &[NavigatorDisplayLine],
    frame: &mut Frame,
    body: Rect,
) {
    let Some(nav) = app.navigator() else {
        return;
    };

    let start = nav.scroll.min(lines.len());
    let end = lines.len().min(start.saturating_add(body.height as usize));
    for (visible_idx, line) in lines[start..end].iter().enumerate() {
        let NavigatorDisplayLine::Row(idx) = *line else {
            continue;
        };
        let y = body.y + visible_idx as u16;
        let rect = Rect::new(body.x, y, body.width, 1);
        let selected = idx == nav.selected;
        render_row(app, frame, rect, rows, idx, selected);
    }
}

fn render_row(
    app: &AppState,
    frame: &mut Frame,
    rect: Rect,
    rows: &[NavigatorRow],
    idx: usize,
    selected: bool,
) {
    let Some(nav) = app.navigator() else {
        return;
    };

    let row = &rows[idx];
    let p = &app.palette;
    frame.render_widget(Clear, rect);
    let base_style = if selected {
        Style::default().bg(p.accent).fg(panel_contrast_fg(p))
    } else {
        Style::default().bg(p.panel_bg).fg(p.text)
    };
    let dim_style = if selected {
        base_style
    } else {
        Style::default().fg(p.overlay0).bg(p.panel_bg)
    };
    let filter_active = nav.state_filter.is_some() || !nav.search.query.text().trim().is_empty();
    let context_only = filter_active && !row.matched;
    let text_style = if selected {
        base_style.add_modifier(Modifier::BOLD)
    } else if context_only {
        let dimmed = Style::default().fg(p.overlay0).bg(p.panel_bg);
        if row.is_workspace {
            dimmed.add_modifier(Modifier::BOLD)
        } else {
            dimmed
        }
    } else if row.is_workspace {
        Style::default()
            .fg(p.accent)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD)
    } else if row.is_current {
        Style::default()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.subtext0).bg(p.panel_bg)
    };
    let (status_icon, status_style) = state_icon(
        row.status,
        row.seen,
        &app.state_icon_symbols(),
        &app.state_icon_colors(),
    );
    let status_style = if selected {
        base_style.add_modifier(Modifier::BOLD)
    } else if context_only {
        Style::default().fg(p.overlay0).bg(p.panel_bg)
    } else {
        status_style.bg(p.panel_bg)
    };

    let prefix = tree_prefix(rows, idx);
    let current = if row.is_current { "◆" } else { " " };
    let gutter = format!(" {current} ");
    let gutter_style = if selected {
        base_style
    } else if row.is_current {
        Style::default().fg(p.accent).bg(p.panel_bg)
    } else {
        dim_style
    };
    // Branch glyphs recede one shade below the workspace caret so the
    // structure stays behind the labels.
    let tree_style = if selected {
        base_style
    } else if row.is_workspace {
        dim_style
    } else {
        Style::default().fg(p.surface1).bg(p.panel_bg)
    };
    // The picker stages a pane by its public identifier, so a row about to be
    // staged leads with the identifier it would store. The goto purpose leaves
    // it off: there the row is a place to go, not a value to record.
    let public_id = (nav.purpose == NavigatorPurpose::PaneTodoLink)
        .then(|| row.public_pane_id.clone())
        .flatten()
        .map(|id| format!("{id} "))
        .unwrap_or_default();
    let meta_width = status_width(
        nav.status_width,
        nav.content_width
            .saturating_sub(nav.status_width)
            .saturating_sub(2),
        rect.width,
    );
    let left_budget = rect
        .width
        .saturating_sub(meta_width)
        .saturating_sub(display_width_u16(&format!("{gutter}{prefix} ")))
        .saturating_sub(display_width_u16(&public_id))
        .saturating_sub(3) as usize;
    let title = truncate_end(&row.label, left_budget);

    let spans = vec![
        Span::styled(gutter, gutter_style),
        Span::styled(prefix, tree_style),
        Span::styled(" ", base_style),
        Span::styled(status_icon, status_style),
        Span::raw(" "),
        Span::styled(public_id, if selected { base_style } else { dim_style }),
        Span::styled(title, text_style),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)).style(base_style), rect);

    if meta_width > 0 {
        let meta_rect = Rect::new(
            rect.x + rect.width.saturating_sub(meta_width),
            rect.y,
            meta_width,
            1,
        );
        let meta = truncate_end(&row.meta, meta_width.saturating_sub(2) as usize);
        let meta_style = if selected {
            base_style
        } else if context_only || row.is_workspace || row.is_tab {
            Style::default().fg(p.overlay0).bg(p.panel_bg)
        } else {
            Style::default()
                .fg(state_label_color(
                    row.status,
                    row.seen,
                    &app.state_icon_colors(),
                ))
                .bg(p.panel_bg)
        };
        frame.render_widget(
            Paragraph::new(format!(" {meta}")).style(meta_style),
            meta_rect,
        );
    }
}

/// Tree prefix for a navigator row: expand caret for workspaces, connected
/// branch glyphs for children (`├──`, `└──` for the last sibling, with `│`
/// continuation lines under ancestors that have more siblings below).
fn tree_prefix(rows: &[NavigatorRow], idx: usize) -> String {
    let row = &rows[idx];
    if row.is_workspace {
        return if row.expanded { "▾" } else { "▸" }.to_string();
    }
    if row.depth == 0 {
        return "  ".to_string();
    }
    let mut prefix = String::new();
    for level in 1..row.depth {
        prefix.push_str(if has_following_sibling_at_depth(rows, idx, level) {
            "│  "
        } else {
            "   "
        });
    }
    prefix.push_str(if has_following_sibling_at_depth(rows, idx, row.depth) {
        "├──"
    } else {
        "└──"
    });
    prefix
}

/// Whether another row at `depth` follows `idx` before the subtree at that
/// depth ends (a row shallower than `depth` closes the subtree).
fn has_following_sibling_at_depth(rows: &[NavigatorRow], idx: usize, depth: u8) -> bool {
    rows[idx + 1..]
        .iter()
        .take_while(|row| row.depth >= depth)
        .any(|row| row.depth == depth)
}

fn render_navigator_scrollbar(app: &AppState, line_count: usize, frame: &mut Frame, body: Rect) {
    let Some(nav) = app.navigator() else {
        return;
    };

    if body.width <= 1 || body.height == 0 {
        return;
    }
    let viewport = body.height as usize;
    if line_count <= viewport {
        return;
    }
    let metrics = crate::pane::ScrollMetrics {
        viewport_rows: viewport,
        offset_from_bottom: line_count
            .saturating_sub(viewport)
            .saturating_sub(nav.scroll),
        max_offset_from_bottom: line_count.saturating_sub(viewport),
    };
    if !should_show_scrollbar(metrics) {
        return;
    }
    let track = Rect::new(body.x + body.width - 1, body.y, 1, body.height);
    render_scrollbar(
        frame,
        metrics,
        track,
        app.palette.surface_dim,
        app.palette.overlay0,
        "▕",
    );
}

/// The navigator's narrowest: its list-focus footer, `esc close` included,
/// plus the border — the widest of its two footers — however short the
/// session's names are. A navigator measured narrower than its own hints
/// would cut off the one that says how to leave.
pub(crate) const NAVIGATOR_MIN_WIDTH: u16 = 73;

/// The widest the status column may measure, its padding included, so one
/// very long agent name cannot eat the box.
pub(crate) const NAVIGATOR_STATUS_MAX_COLUMNS: u16 = 40;

/// What the label side of a row keeps before the status column is cut: the
/// gutter, the deepest tree prefix, the icon and 16 columns of label.
const NAVIGATOR_LABEL_FLOOR: u16 = 3 + 6 + 3 + 16;

/// The longest state word a status can end in. A status is measured as if it
/// ended in this one, so a pane turning from `idle` to `working` while the
/// navigator is open still fits the column it was given.
const NAVIGATOR_LONGEST_STATE_WORD: usize = 7;

/// The two columns the navigator's rows ask for, measured once when it opens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NavigatorColumns {
    /// The widest row's gutter, tree prefix, icon, identifier and label, with
    /// a spare column.
    pub label: u16,
    /// The widest status, a space before it and a spare after; 0 when no row
    /// has one.
    pub status: u16,
}

impl NavigatorColumns {
    /// The box these columns want, border included.
    pub(crate) fn content_width(self) -> u16 {
        self.label.saturating_add(self.status).saturating_add(2)
    }
}

/// Measure the navigator's rows: the same parts [`render_row`] draws, so a
/// row measured to fit is drawn whole.
///
/// Measured over the rows as they are when the navigator opens (every space
/// expanded, nothing filtered) and kept, so the box does not change width
/// while a query narrows the list or a space is folded.
pub(crate) fn navigator_columns(
    rows: &[NavigatorRow],
    purpose: NavigatorPurpose,
) -> NavigatorColumns {
    let label = (0..rows.len())
        .map(|idx| {
            let row = &rows[idx];
            let public_id = if purpose == NavigatorPurpose::PaneTodoLink {
                row.public_pane_id
                    .as_deref()
                    .map(|id| display_width_u16(id) as usize + 1)
                    .unwrap_or(0)
            } else {
                0
            };
            // gutter, prefix and its space; icon, its space and one spare
            3 + display_width_u16(&tree_prefix(rows, idx)) as usize
                + 1
                + 3
                + public_id
                + display_width_u16(&row.label) as usize
        })
        .max()
        .unwrap_or(0);
    let status = rows
        .iter()
        .map(|row| status_measure(&row.meta))
        .max()
        .unwrap_or(0);
    let status = if status == 0 {
        0
    } else {
        (status + 2).min(usize::from(NAVIGATOR_STATUS_MAX_COLUMNS))
    };
    NavigatorColumns {
        label: label.min(usize::from(u16::MAX)) as u16,
        status: status as u16,
    }
}

/// A status's width with its state word counted at the longest a state word
/// can be. Statuses that do not end in a built-in state word are measured as
/// they are.
fn status_measure(meta: &str) -> usize {
    const STATE_WORDS: [&str; 5] = ["blocked", "working", "done", "idle", "unknown"];
    let width = display_width_u16(meta) as usize;
    STATE_WORDS
        .iter()
        .find(|word| {
            meta.strip_suffix(**word)
                .is_some_and(|rest| rest.ends_with(" · "))
        })
        .map(|word| width - word.len() + NAVIGATOR_LONGEST_STATE_WORD)
        .unwrap_or(width)
}

/// The status column a row of `width` draws: the measured one, unless the
/// row is too narrow for it and its labels, in which case the status gives way
/// only after the labels have come down to their floor. Labels narrower than
/// the floor keep only what they measured.
fn status_width(measured: u16, label: u16, width: u16) -> u16 {
    measured.min(width.saturating_sub(NAVIGATOR_LABEL_FLOOR.min(label)))
}

fn render_detail(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    frame: &mut Frame,
    area: Rect,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    render_separator(frame, area, app);
    let detail = selected_detail(app, terminal_runtimes);
    if detail.is_empty() {
        return;
    }
    let text = middle_elide(&detail, area.width.saturating_sub(2) as usize);
    frame.render_widget(
        Paragraph::new(format!(" {text}")).style(Style::default().fg(app.palette.overlay0)),
        area,
    );
}

fn selected_detail(app: &AppState, terminal_runtimes: &TerminalRuntimeRegistry) -> String {
    let Some(nav) = app.navigator() else {
        return String::new();
    };

    let rows = app.navigator_rows_from(terminal_runtimes);
    let Some(row) = rows.get(nav.selected) else {
        return String::new();
    };
    match row.target {
        NavigatorTarget::Workspace { ws_idx } => workspace_detail(app, terminal_runtimes, ws_idx),
        NavigatorTarget::Tab { ws_idx, tab_idx } => {
            tab_detail(app, terminal_runtimes, ws_idx, tab_idx)
        }
        NavigatorTarget::Pane {
            ws_idx,
            tab_idx,
            pane_id,
        } => pane_detail(app, terminal_runtimes, ws_idx, tab_idx, pane_id),
        NavigatorTarget::ClearLink => "leave this todo with no link".to_string(),
    }
}

fn workspace_detail(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    ws_idx: usize,
) -> String {
    let Some(ws) = app.workspaces.get(ws_idx) else {
        return String::new();
    };
    let label = ws.display_name_from(&app.terminals, terminal_runtimes);
    let pane_count = ws.tabs.iter().map(|tab| tab.panes.len()).sum::<usize>();
    let mut parts = vec![label, crate::ui::text::pane_count(pane_count)];
    if !rowless_workspace_activity(app, terminal_runtimes, ws_idx).is_empty() {
        parts.push(rowless_workspace_activity(app, terminal_runtimes, ws_idx));
    }
    parts.join(" · ")
}

fn tab_detail(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    ws_idx: usize,
    tab_idx: usize,
) -> String {
    let Some(ws) = app.workspaces.get(ws_idx) else {
        return String::new();
    };
    let Some(tab) = ws.tabs.get(tab_idx) else {
        return String::new();
    };
    let mut parts = vec![
        ws.display_name_from(&app.terminals, terminal_runtimes),
        format!(
            "tab: {}",
            ws.tab_display_name(tab_idx)
                .unwrap_or_else(|| (tab_idx + 1).to_string())
        ),
        crate::ui::text::pane_count(tab.panes.len()),
    ];
    let rows = app.navigator_rows_from(terminal_runtimes);
    if let Some(meta) = rows
        .into_iter()
        .find(|row| matches!(row.target, NavigatorTarget::Tab { ws_idx: row_ws_idx, tab_idx: row_tab_idx } if row_ws_idx == ws_idx && row_tab_idx == tab_idx))
        .map(|row| row.meta)
        .filter(|meta| !meta.is_empty())
    {
        parts.push(meta);
    }
    parts.join(" · ")
}

fn pane_detail(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    ws_idx: usize,
    tab_idx: usize,
    pane_id: crate::layout::PaneId,
) -> String {
    let Some(ws) = app.workspaces.get(ws_idx) else {
        return String::new();
    };
    let Some(tab) = ws.tabs.get(tab_idx) else {
        return String::new();
    };
    let mut parts = vec![ws.display_name_from(&app.terminals, terminal_runtimes)];
    if ws.tabs.len() > 1 {
        parts.push(format!(
            "tab: {}",
            ws.tab_display_name(tab_idx)
                .unwrap_or_else(|| (tab_idx + 1).to_string())
        ));
    }
    if let Some(pane_number) = ws.public_pane_number(pane_id) {
        parts.push(format!("pane {pane_number}"));
    }
    if let Some(terminal_id) = tab.terminal_id(pane_id) {
        if let Some(terminal) = app.terminals.get(terminal_id) {
            let presentation = terminal.effective_presentation();
            if let Some(title) = presentation.title {
                parts.push(title);
            }
            let display_agent = terminal.effective_display_agent();
            if let Some(agent) = display_agent.as_deref().or_else(|| {
                terminal
                    .agent_name
                    .as_deref()
                    .or_else(|| terminal.effective_agent_label())
            }) {
                parts.push(agent.to_string());
                let seen = tab
                    .panes
                    .get(&pane_id)
                    .map(|pane| pane.seen)
                    .unwrap_or(true);
                let state = row_state(app, ws_idx, tab_idx, pane_id);
                let status = presentation
                    .state_labels
                    .get(display_state(state, seen))
                    .cloned()
                    .unwrap_or_else(|| display_state(state, seen).to_string());
                parts.push(status);
            } else {
                parts.push("shell".to_string());
            }
        }
    }
    parts.join(" · ")
}

fn rowless_workspace_activity(
    app: &AppState,
    terminal_runtimes: &TerminalRuntimeRegistry,
    ws_idx: usize,
) -> String {
    app.navigator_rows_from(terminal_runtimes)
        .into_iter()
        .find(|row| matches!(row.target, NavigatorTarget::Workspace { ws_idx: row_ws_idx } if row_ws_idx == ws_idx))
        .map(|row| row.meta)
        .unwrap_or_default()
}

fn row_state(
    app: &AppState,
    ws_idx: usize,
    tab_idx: usize,
    pane_id: crate::layout::PaneId,
) -> crate::detect::AgentState {
    app.workspaces
        .get(ws_idx)
        .and_then(|ws| ws.tabs.get(tab_idx))
        .and_then(|tab| tab.terminal_id(pane_id))
        .and_then(|terminal_id| app.terminals.get(terminal_id))
        .map(|terminal| terminal.state)
        .unwrap_or(crate::detect::AgentState::Unknown)
}

fn display_state(state: crate::detect::AgentState, seen: bool) -> &'static str {
    match (state, seen) {
        (crate::detect::AgentState::Blocked, _) => "blocked",
        (crate::detect::AgentState::Working, _) => "working",
        (crate::detect::AgentState::Idle, false) => "done",
        (crate::detect::AgentState::Idle, true) => "idle",
        (crate::detect::AgentState::Unknown, _) => "unknown",
    }
}

fn render_footer(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(nav) = app.navigator() else {
        return;
    };

    if area.height == 0 {
        return;
    }
    let p = &app.palette;
    let key = Style::default().fg(p.accent).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(p.overlay0);
    let line = if nav.search.focused {
        Line::from(vec![
            Span::styled(" enter", key),
            Span::styled(
                if nav.purpose == NavigatorPurpose::PaneTodoLink {
                    " link  "
                } else {
                    " switch  "
                },
                dim,
            ),
            Span::styled("^j/^k/↑↓", key),
            Span::styled(" move  ", dim),
            Span::styled("ctrl+u", key),
            Span::styled(" clear  ", dim),
            Span::styled("esc", key),
            Span::styled(" back", dim),
        ])
    } else {
        Line::from(vec![
            Span::styled(" enter", key),
            Span::styled(
                if nav.purpose == NavigatorPurpose::PaneTodoLink {
                    " link  "
                } else {
                    " switch  "
                },
                dim,
            ),
            Span::styled("/", key),
            Span::styled(" search  ", dim),
            Span::styled("b/w/i/d/a", key),
            Span::styled(" states  ", dim),
            Span::styled("j/k/^j/^k/↑↓", key),
            Span::styled(" move  ", dim),
            Span::styled("esc", key),
            Span::styled(" close", dim),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::AgentState;

    fn row(depth: u8, is_workspace: bool) -> NavigatorRow {
        NavigatorRow {
            target: NavigatorTarget::Workspace { ws_idx: 0 },
            depth,
            label: String::new(),
            meta: String::new(),
            status: AgentState::Idle,
            seen: true,
            is_current: false,
            is_workspace,
            is_tab: false,
            expanded: true,
            public_pane_id: None,
            search_text: String::new(),
            matched: true,
        }
    }

    fn multi_tab_rows() -> Vec<NavigatorRow> {
        vec![
            row(0, true),  // workspace
            row(1, false), // tab a
            row(2, false), // pane
            row(2, false), // pane (last in tab a)
            row(1, false), // tab b (last tab)
            row(2, false), // pane (last in tab b)
            row(0, true),  // workspace
            row(1, false), // pane (single child)
        ]
    }

    #[test]
    fn workspace_rows_use_expand_caret() {
        let rows = multi_tab_rows();
        assert_eq!(tree_prefix(&rows, 0), "▾");
        let mut collapsed = rows.clone();
        collapsed[0].expanded = false;
        assert_eq!(tree_prefix(&collapsed, 0), "▸");
    }

    #[test]
    fn middle_children_get_branch_glyph() {
        let rows = multi_tab_rows();
        assert_eq!(tree_prefix(&rows, 1), "├──");
        assert_eq!(tree_prefix(&rows, 2), "│  ├──");
    }

    #[test]
    fn last_children_get_terminator_glyph() {
        let rows = multi_tab_rows();
        assert_eq!(tree_prefix(&rows, 3), "│  └──");
        assert_eq!(tree_prefix(&rows, 4), "└──");
        assert_eq!(tree_prefix(&rows, 7), "└──");
    }

    #[test]
    fn spine_stops_after_last_ancestor_sibling() {
        let rows = multi_tab_rows();
        assert_eq!(tree_prefix(&rows, 5), "   └──");
    }

    #[test]
    fn next_workspace_does_not_extend_previous_subtree() {
        // The pane at idx 5 is last in its workspace even though another
        // workspace with children follows.
        let rows = multi_tab_rows();
        assert!(!has_following_sibling_at_depth(&rows, 5, 1));
        assert!(!has_following_sibling_at_depth(&rows, 5, 2));
    }

    /// #105: in a 310x56 window the navigator was 272 columns wide, most of
    /// it empty. It measures its rows now, and stops at the shared cap.
    #[test]
    fn navigator_is_at_most_120_wide_in_a_310x56_window() {
        let mut app = crate::ui::test_support::app_with_one_pane(&"a-long-space-name ".repeat(12));
        crate::ui::test_support::layout_sized(&mut app, 310, 56);
        app.open_navigator();

        let popup = app.navigator_popup_rect();

        assert_eq!(popup.width, crate::ui::overlay::LIST_DIALOG_MAX_WIDTH);
        assert_eq!(popup.x, (310 - popup.width) / 2, "centred");

        // A short list is narrower still, down to the navigator's floor.
        let mut short = crate::ui::test_support::app_with_one_pane("ctx");
        crate::ui::test_support::layout_sized(&mut short, 310, 56);
        short.open_navigator();
        let popup = short.navigator_popup_rect();
        assert_eq!(popup.width, NAVIGATOR_MIN_WIDTH);
        assert_eq!(popup.x, (310 - NAVIGATOR_MIN_WIDTH) / 2);
    }

    fn name_the_pane_agent(app: &mut AppState, name: &str) {
        let terminal_id = app.workspaces[0]
            .pane_state(app.workspaces[0].tabs[0].root_pane)
            .expect("pane")
            .attached_terminal_id
            .clone();
        let terminal = app.terminals.get_mut(&terminal_id).expect("terminal");
        terminal.agent_name = Some(name.into());
        terminal.state = AgentState::Idle;
    }

    fn navigator_rows_text(app: &AppState, width: u16, height: u16) -> Vec<String> {
        let buffer = crate::ui::test_support::draw_sized(app, width, height);
        crate::ui::test_support::rect_rows(&buffer, app.navigator_popup_rect())
    }

    /// #109: the box was sized for a 28-column status column and then drawn
    /// with 20, so `keyboard-shortcuts · idle` came out `keyboard-shortcut…`.
    #[test]
    fn navigator_draws_the_status_column_it_measured() {
        let mut app = crate::ui::test_support::app_with_one_pane("macOS");
        name_the_pane_agent(&mut app, "keyboard-shortcuts");
        crate::ui::test_support::layout_sized(&mut app, 310, 56);
        app.open_navigator();
        crate::ui::test_support::layout_sized(&mut app, 310, 56);

        let rows = navigator_rows_text(&app, 310, 56);

        assert!(
            rows.iter()
                .any(|row| row.contains("keyboard-shortcuts · idle")),
            "the status is drawn whole: {rows:#?}"
        );
        assert!(!rows.iter().any(|row| row.contains('…')), "{rows:#?}");
    }

    #[test]
    fn navigator_status_width_counts_the_longest_state_word() {
        let mut idle = row(1, false);
        idle.meta = "claude · idle".into();
        let mut shell = row(1, false);
        shell.meta = "shell".into();

        let columns = navigator_columns(&[idle, shell], NavigatorPurpose::Goto);

        // `claude · working` is 16; a space before it and one spare after.
        assert_eq!(columns.status, 16 + 2);
    }

    #[test]
    fn navigator_status_width_is_capped_at_40() {
        let mut long = row(1, false);
        long.meta = format!("{} · idle", "x".repeat(80));

        let columns = navigator_columns(&[long], NavigatorPurpose::Goto);

        assert_eq!(columns.status, NAVIGATOR_STATUS_MAX_COLUMNS);
    }

    #[test]
    fn navigator_cuts_labels_before_statuses_in_a_small_window() {
        let mut app = crate::ui::test_support::app_with_one_pane(&"a-long-space-name-".repeat(4));
        name_the_pane_agent(&mut app, "keyboard-shortcuts");
        crate::ui::test_support::layout_sized(&mut app, 60, 30);
        app.open_navigator();
        crate::ui::test_support::layout_sized(&mut app, 60, 30);

        let rows = navigator_rows_text(&app, 60, 30);

        assert!(
            rows.iter()
                .any(|row| row.contains("keyboard-shortcuts · idle")),
            "{rows:#?}"
        );
        assert!(
            rows.iter().any(|row| row.contains('…')),
            "the long space name gave way: {rows:#?}"
        );
    }

    /// #116: labels narrower than the floor never needed all of it, so a box
    /// sized to labels + status must draw the status whole.
    #[test]
    fn a_status_is_not_cut_for_a_label_floor_the_labels_do_not_need() {
        assert_eq!(status_width(38, 20, 20 + 38), 38);
        // Labels wider than the floor keep the floor when the box is short.
        assert_eq!(status_width(38, 40, 50), 50 - NAVIGATOR_LABEL_FLOOR);
    }

    #[test]
    fn a_count_of_one_pane_is_singular() {
        assert_eq!(crate::ui::text::pane_count(1), "1 pane");
        assert_eq!(crate::ui::text::pane_count(3), "3 panes");
    }

    #[test]
    fn snapshot_navigator() {
        crate::ui::test_support::overlay_snapshot_of(|app| app.open_navigator()).assert(
            Rect::new(3, 2, 73, 21),
            &[
                "┌───────────────────────────────────────────────────────────────────────┐",
                "│ / search panes                                                 1 pane │",
                "│───────────────────────────────────────────────────────────────────────│",
                "│ ◆ ▾ · overlay (1)                                                     │",
                "│ ◆ └── · pane 1                                                  shell │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│                                                                       │",
                "│ overlay · pane 1 · shell──────────────────────────────────────────────│",
                "│ enter switch  / search  b/w/i/d/a states  j/k/^j/^k/↑↓ move  esc close│",
                "└───────────────────────────────────────────────────────────────────────┘",
            ],
        );
    }
}
