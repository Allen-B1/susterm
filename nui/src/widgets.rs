use crate::{Format, Widget, ScreenBuffer};

pub struct Entry {
    pub x: usize, pub y: usize,
    pub format: Format,
    pub text: Vec<u8>,
    pub max: usize,
}

impl Widget for Entry {
    fn draw(&self, buf: &mut ScreenBuffer) -> (usize, usize) {
        for (i, &ch) in self.text.iter().enumerate(){
            let x = self.x + i;
            if x >= buf.width {
                break
            }

            let idx = x + self.y * buf.width;
            buf.chars[idx] = ch;
            buf.formats[idx] = self.format;
        }
        for i in self.text.len()..self.max {
            let x = self.x + i;
            if x >= buf.width {
                break
            }

            let idx = x + self.y * buf.width;
            buf.chars[idx] = ' ' as u8;
            buf.formats[idx] = self.format;
        }

        (self.x + self.text.len(), self.y)
    }

    fn focusable(&self) -> bool {
        return true
    }

    fn keypress(&mut self, ch: u8) {
        if ch == 8 || ch == 127 {
            self.text.pop();
        } else {
            if self.text.len() < self.max {
                self.text.push(ch);
            }
        }
    }
}
