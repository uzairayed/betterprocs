pub struct TerminalScreen {
    parser: vt100::Parser,
    pub scroll_offset: usize,
    rows: u16,
    cols: u16,
}

impl TerminalScreen {
    pub fn new(rows: u16, cols: u16, scrollback: usize) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, scrollback),
            scroll_offset: 0,
            rows,
            cols,
        }
    }

    pub fn process_bytes(&mut self, data: &[u8]) {
        self.parser.process(data);
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// Return the last `max_lines` physical lines of output, including
    /// scrollback. The visible scroll position is preserved.
    ///
    /// We read the buffer one physical row at a time (`Screen::rows`) rather
    /// than `contents()` so that soft-wrapped rows stay aligned to a stable
    /// absolute index as we step the scrollback window — `contents()` joins
    /// wrapped rows, which would shift the alignment between windows.
    pub fn tail_text(&mut self, max_lines: usize) -> String {
        let saved = self.parser.screen().scrollback();
        let height = self.rows as usize;
        let width = self.cols;
        // `Screen` doesn't expose the scrollback length directly, but
        // `set_scrollback` clamps to it, so the max reachable offset is the
        // total number of scrollback rows above the live screen.
        self.parser.screen_mut().set_scrollback(usize::MAX);
        let total_back = self.parser.screen().scrollback();
        let total = total_back + height;

        let mut buf: Vec<String> = vec![String::new(); total];

        // Visit windows from the oldest scrollback offset down to 0 (live
        // screen), stepping by one screen height so the windows tile the
        // whole buffer contiguously.
        let mut offset = total_back as isize;
        loop {
            let scrollback = offset.max(0) as usize;
            self.parser.screen_mut().set_scrollback(scrollback);
            let base = total_back - scrollback; // absolute index of window top
            for (i, line) in self.parser.screen().rows(0, width).enumerate() {
                if let Some(slot) = buf.get_mut(base + i) {
                    *slot = line;
                }
            }
            if offset <= 0 {
                break;
            }
            offset -= height as isize;
        }

        // Restore the caller's scroll position.
        self.parser.screen_mut().set_scrollback(saved);

        // Drop trailing blank lines, then keep only the last `max_lines`.
        while buf.last().is_some_and(|l| l.trim_end().is_empty()) {
            buf.pop();
        }
        let start = buf.len().saturating_sub(max_lines);
        buf[start..].join("\n")
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.parser.screen_mut().set_size(rows, cols);
    }

    pub fn clear(&mut self) {
        self.parser = vt100::Parser::new(self.rows, self.cols, 10_000);
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset += n;
        self.apply_scroll();
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
        self.apply_scroll();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.apply_scroll();
    }

    fn apply_scroll(&mut self) {
        // set_scrollback clamps to the actual scrollback buffer length internally,
        // so we don't need to know the max — just set what we want.
        self.parser.screen_mut().set_scrollback(self.scroll_offset);
        // Read back the clamped value so our offset stays in bounds.
        self.scroll_offset = self.parser.screen().scrollback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_text_includes_scrollback_beyond_screen() {
        // 3-row screen, but write 10 lines — 7 must live in scrollback.
        let mut screen = TerminalScreen::new(3, 20, 100);
        for i in 0..10 {
            screen.process_bytes(format!("line{}\r\n", i).as_bytes());
        }
        let all = screen.tail_text(100);
        for i in 0..10 {
            assert!(all.contains(&format!("line{}", i)), "missing line{} in:\n{}", i, all);
        }
    }

    #[test]
    fn tail_text_limits_to_max_lines() {
        let mut screen = TerminalScreen::new(3, 20, 100);
        for i in 0..10 {
            screen.process_bytes(format!("line{}\r\n", i).as_bytes());
        }
        let last_two = screen.tail_text(2);
        let lines: Vec<&str> = last_two.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(last_two.contains("line9"));
        assert!(last_two.contains("line8"));
        assert!(!last_two.contains("line7"));
    }

    #[test]
    fn tail_text_preserves_scroll_position() {
        let mut screen = TerminalScreen::new(3, 20, 100);
        for i in 0..10 {
            screen.process_bytes(format!("line{}\r\n", i).as_bytes());
        }
        screen.scroll_up(2);
        let before = screen.scroll_offset;
        let _ = screen.tail_text(5);
        assert_eq!(screen.scroll_offset, before);
        assert_eq!(screen.screen().scrollback(), before);
    }
}
