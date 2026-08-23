//! One definition of where an anchored panel sits.
//!
//! The notification center and the pane todo panel each used to carry ~60 lines
//! performing the identical sequence — pick an anchor, clamp a measured width,
//! count rows, reserve a footer, place x and y against the screen, then derive
//! the inner rect, the list rect and the footer row from the result. They
//! differed in what they measured and where they anchored, and in nothing else.

use ratatui::layout::Rect;

use crate::ui::widgets::footer_split;

/// Where the panel's top edge comes from. Horizontal placement is not a choice:
/// both panels right-align to their anchor, so the resolver does that and this
/// enum stays about the axis that actually varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerticalAnchor {
    /// Hangs under the anchor's bottom edge — a dropdown from a tab bar.
    Below,
    /// Starts one row inside the anchor's top edge, so it drops out of a
    /// border indicator rather than covering it.
    InsideTop,
    /// Opens above this rect's top edge, so the thing that toggles the panel
    /// stays visible underneath it (the global-launcher idiom). An empty rect
    /// means there is nothing to clear, and the panel sits flush at the bottom.
    Above(Rect),
}

/// What a caller knows about its own panel. Everything here is either constant
/// per panel or already measured at the call site.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AnchoredPanelSpec {
    /// The panel right-aligns to this rect's right edge.
    pub anchor: Rect,
    /// Everything the panel may be placed within.
    pub screen: Rect,
    /// The panel's desired outer width: what the caller measured its own rows
    /// to need, its own chrome included. Measuring is the one genuinely
    /// per-panel thing — one panel counts a title plus a context plus an age
    /// column, the other a label plus a link chip — so it stays with the
    /// caller and everything downstream of it does not.
    pub content_width: u16,
    /// Inclusive clamp on the resolved width, before the screen has its say.
    pub width_bounds: (u16, u16),
    /// Content rows the panel would like. Always at least one, so an empty
    /// panel is still a panel.
    pub rows: u16,
    /// Cap on `rows`.
    pub max_rows: u16,
    /// Rows reserved below the list, normally [`crate::ui::FOOTER_ROWS`]. Zero
    /// for a panel with no footer to show.
    pub footer_rows: u16,
    /// Rows reserved between the list and the footer for a detail block, rule
    /// row included. Zero for a panel that shows no detail, which is every
    /// panel whose selection has nothing more to say — the block appears only
    /// when it has something to hold.
    pub detail_rows: u16,
    /// Where the panel's top edge comes from.
    pub vertical: VerticalAnchor,
}

/// Everything an anchored panel is drawn and hit-tested against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PanelGeometry {
    /// The panel including its border.
    pub outer: Rect,
    /// The panel minus its border: list and footer together.
    pub inner: Rect,
    /// The list rect, stopping short of the footer block.
    pub list: Rect,
    /// The footer's button row, absent when there is no room for the whole
    /// footer block or nothing to put in it.
    pub footer_row: Option<Rect>,
    /// The detail block between list and footer, absent when the panel asked
    /// for none or there was no room. Its first row is a rule.
    pub detail: Option<Rect>,
}

impl AnchoredPanelSpec {
    /// The width `resolve` will settle on, for a caller that has to lay text
    /// out inside the panel before it can say how many rows that text needs.
    pub(crate) fn resolved_width(&self) -> u16 {
        let (min_width, max_width) = self.width_bounds;
        self.content_width
            .clamp(min_width, max_width)
            .min(self.screen.width.max(1))
    }

    /// Resolve the panel's placement, or `None` when there is no screen to
    /// place it on — render and hit-test go quiet together.
    pub(crate) fn resolve(&self) -> Option<PanelGeometry> {
        let screen = self.screen;
        if screen.width == 0 || screen.height == 0 {
            return None;
        }

        let width = self.resolved_width();
        let rows = self.rows.max(1).min(self.max_rows);
        let height = (rows + 2 + self.footer_rows + self.detail_rows).min(screen.height.max(1));

        let right = self.anchor.x.saturating_add(self.anchor.width);
        let x = right.saturating_sub(width).max(screen.x);

        // The lowest top edge that still leaves room for the whole panel.
        let bottom_y = screen.y + screen.height.saturating_sub(height);
        let top = match self.vertical {
            VerticalAnchor::Below => self.anchor.y.saturating_add(self.anchor.height),
            VerticalAnchor::InsideTop => self.anchor.y.saturating_add(1),
            VerticalAnchor::Above(over) if over.width > 0 => over.y.saturating_sub(height),
            VerticalAnchor::Above(_) => bottom_y,
        };
        let y = top.min(bottom_y).max(screen.y);

        let outer = Rect::new(x, y, width, height);
        let inner = Rect::new(
            outer.x + 1,
            outer.y + 1,
            outer.width.saturating_sub(2),
            outer.height.saturating_sub(2),
        );
        // A footer that cannot fit its whole block, blank row included, is not
        // drawn — the buttons would otherwise sit flush against the last row.
        let has_footer =
            self.footer_rows > 0 && inner.width > 0 && inner.height >= self.footer_rows;
        let (list, _) = footer_split(inner, has_footer);
        let footer_row =
            has_footer.then(|| Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1));

        // The detail block is carved off the bottom of the list, directly
        // above the footer. It yields to the list rather than the other way
        // round: a panel squeezed to nothing shows its todos, not a detail of
        // one of them.
        let detail_rows = self.detail_rows.min(list.height.saturating_sub(1));
        let (list, detail) = if detail_rows > 0 {
            (
                Rect::new(list.x, list.y, list.width, list.height - detail_rows),
                Some(Rect::new(
                    list.x,
                    list.y + list.height - detail_rows,
                    list.width,
                    detail_rows,
                )),
            )
        } else {
            (list, None)
        };

        Some(PanelGeometry {
            outer,
            inner,
            list,
            footer_row,
            detail,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 25,
    };

    fn spec(vertical: VerticalAnchor) -> AnchoredPanelSpec {
        AnchoredPanelSpec {
            anchor: Rect::new(0, 0, 80, 1),
            screen: SCREEN,
            content_width: 40,
            width_bounds: (30, 60),
            rows: 3,
            max_rows: 12,
            footer_rows: crate::ui::FOOTER_ROWS,
            detail_rows: 0,
            vertical,
        }
    }

    #[test]
    fn width_clamps_between_its_bounds_then_to_the_screen() {
        let narrow = AnchoredPanelSpec {
            content_width: 4,
            ..spec(VerticalAnchor::Below)
        };
        assert_eq!(narrow.resolve().expect("resolves").outer.width, 30);

        let wide = AnchoredPanelSpec {
            content_width: 500,
            ..spec(VerticalAnchor::Below)
        };
        assert_eq!(wide.resolve().expect("resolves").outer.width, 60);

        let cramped = AnchoredPanelSpec {
            screen: Rect::new(0, 0, 20, 25),
            anchor: Rect::new(0, 0, 20, 1),
            ..wide
        };
        assert_eq!(cramped.resolve().expect("resolves").outer.width, 20);
    }

    #[test]
    fn rows_are_at_least_one_and_capped() {
        let empty = AnchoredPanelSpec {
            rows: 0,
            ..spec(VerticalAnchor::Below)
        };
        // one row, two borders, and the footer block
        assert_eq!(empty.resolve().expect("resolves").outer.height, 5);

        let many = AnchoredPanelSpec {
            rows: 40,
            ..spec(VerticalAnchor::Below)
        };
        assert_eq!(many.resolve().expect("resolves").outer.height, 16);
    }

    #[test]
    fn the_panel_right_aligns_to_its_anchor() {
        let geometry = AnchoredPanelSpec {
            anchor: Rect::new(0, 0, 50, 1),
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .expect("resolves");
        assert_eq!(geometry.outer.x + geometry.outer.width, 50);

        // An anchor narrower than the panel would push it off the left edge.
        let clamped = AnchoredPanelSpec {
            anchor: Rect::new(0, 0, 10, 1),
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .expect("resolves");
        assert_eq!(clamped.outer.x, SCREEN.x);
    }

    #[test]
    fn below_hangs_under_the_anchor_and_inside_top_drops_one_row_in() {
        let anchor = Rect::new(0, 4, 60, 10);
        let below = AnchoredPanelSpec {
            anchor,
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .expect("resolves");
        assert_eq!(below.outer.y, 14);

        let inside = AnchoredPanelSpec {
            anchor,
            ..spec(VerticalAnchor::InsideTop)
        }
        .resolve()
        .expect("resolves");
        assert_eq!(inside.outer.y, 5);
    }

    #[test]
    fn above_opens_over_the_rect_that_toggles_it() {
        let indicator = Rect::new(70, 24, 5, 1);
        let above = spec(VerticalAnchor::Above(indicator))
            .resolve()
            .expect("resolves");
        assert_eq!(above.outer.y + above.outer.height, indicator.y);

        // Nothing to open above: flush at the bottom instead.
        let flush = spec(VerticalAnchor::Above(Rect::default()))
            .resolve()
            .expect("resolves");
        assert_eq!(flush.outer.y + flush.outer.height, SCREEN.height);
    }

    #[test]
    fn a_panel_that_would_fall_off_the_bottom_is_pushed_up() {
        let geometry = AnchoredPanelSpec {
            anchor: Rect::new(0, 23, 60, 1),
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .expect("resolves");
        assert_eq!(geometry.outer.y + geometry.outer.height, SCREEN.height);
    }

    #[test]
    fn inner_list_and_footer_partition_the_panel() {
        let geometry = spec(VerticalAnchor::Below).resolve().expect("resolves");
        let outer = geometry.outer;
        assert_eq!(
            geometry.inner,
            Rect::new(outer.x + 1, outer.y + 1, outer.width - 2, outer.height - 2)
        );
        let footer = geometry.footer_row.expect("footer row");
        assert_eq!(footer.y, geometry.inner.y + geometry.inner.height - 1);
        // The list stops a blank row short of the buttons.
        assert_eq!(
            geometry.list.y + geometry.list.height + crate::ui::FOOTER_ROWS,
            geometry.inner.y + geometry.inner.height
        );
        assert_eq!(geometry.list.y + geometry.list.height, footer.y - 1);
    }

    #[test]
    fn a_panel_with_no_footer_gives_the_whole_inner_area_to_its_list() {
        let geometry = AnchoredPanelSpec {
            footer_rows: 0,
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .expect("resolves");
        assert_eq!(geometry.list, geometry.inner);
        assert!(geometry.footer_row.is_none());
    }

    /// Too short for the whole footer block: the buttons are dropped rather
    /// than drawn flush against the last row, and the list keeps the space.
    #[test]
    fn a_footer_that_cannot_fit_its_block_is_not_drawn() {
        let geometry = AnchoredPanelSpec {
            screen: Rect::new(0, 0, 80, 3),
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .expect("resolves");
        assert_eq!(geometry.inner.height, 1);
        assert!(geometry.footer_row.is_none());
        assert_eq!(geometry.list, geometry.inner);
    }

    #[test]
    fn a_panel_with_no_screen_resolves_to_nothing() {
        assert!(AnchoredPanelSpec {
            screen: Rect::new(0, 0, 0, 25),
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .is_none());
        assert!(AnchoredPanelSpec {
            screen: Rect::new(0, 0, 80, 0),
            ..spec(VerticalAnchor::Below)
        }
        .resolve()
        .is_none());
    }
}
