use std::cmp::{max, min};

#[derive(Default)]
pub(super) struct ListState {
    pub(super) selected: usize,
    pub(super) scroll: usize,
    last_page_size: usize,
}

impl ListState {
    pub(super) fn move_by(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        let next = self.selected as isize + delta;
        self.selected = next.clamp(0, len as isize - 1) as usize;
    }

    pub(super) fn jump_to(&mut self, index: usize, len: usize) {
        if len == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = min(index, len - 1);
    }

    pub(super) fn page_up(&mut self, len: usize) {
        let step = max(1, self.last_page_size.saturating_sub(1));
        self.move_by(-(step as isize), len);
    }

    pub(super) fn page_down(&mut self, len: usize) {
        let step = max(1, self.last_page_size.saturating_sub(1));
        self.move_by(step as isize, len);
    }

    pub(super) fn keep_visible(&mut self, visible_rows: usize, len: usize) {
        self.last_page_size = visible_rows;
        if len == 0 || visible_rows == 0 {
            self.scroll = 0;
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        let max_scroll = len.saturating_sub(visible_rows);
        if self.selected >= self.scroll + visible_rows {
            self.scroll = self.selected + 1 - visible_rows;
        }
        self.scroll = min(self.scroll, max_scroll);
    }
}

#[derive(Clone, Copy)]
pub(super) struct PaneRect {
    pub(super) left: usize,
    pub(super) top: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}
