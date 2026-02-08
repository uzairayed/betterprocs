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
