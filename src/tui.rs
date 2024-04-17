use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{
    clear, color,
    cursor::{self, DetectCursorPos},
    style, terminal_size,
};
use toml::map::IterMut;
use tracing::info;

use std::collections::HashMap;
use std::fmt::Display;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use crate::error::Error;

pub struct Window<R, W: Write> {
    stdin: R,
    stdout: W,
    path_surface: Surface,
    width: u16,
    height: u16,
    help: Vec<String>,
    input: String,
    paths: Vec<PathBuf>,
    filtered_paths: Vec<PathBuf>,
}

impl<R: Iterator<Item = std::io::Result<Key>>, W: Write> Window<R, W> {
    pub fn filter_paths<P>(&mut self, filter: P)
    where
        P: FnMut(&PathBuf) -> bool,
    {
        self.filtered_paths =
            self.paths.clone().into_iter().filter(filter).collect();
    }
    pub fn draw_paths(&mut self) -> Result<(), Error> {
        write!(self.stdout, "{}", cursor::Hide)?;
        write!(self.stdout, "{}", style::Reset)?;
        if self.filtered_paths.len() as u16 <= self.path_surface.row_start {
            write!(
                self.stdout,
                "{}{}[{}/{}]",
                cursor::Goto(1, self.path_surface.row_end + 1),
                style::Bold,
                self.filtered_paths.len(),
                self.filtered_paths.len(),
            )?;
        } else {
            write!(
                self.stdout,
                "{}{}[{}/{}]",
                cursor::Goto(1, self.path_surface.row_end + 1),
                style::Bold,
                self.filtered_paths.len(),
                self.path_surface.row_end - self.path_surface.row_start
            )?;
        }
        write!(self.stdout, "{}", color::Fg(color::LightBlack))?;
        let mut line_acc = "".to_string();
        let (col, _) = self.stdout.cursor_pos()?;
        for _ in 0..(self.path_surface.col_end - col) {
            line_acc = format!("{}\u{2014}", line_acc);
        }
        write!(self.stdout, "{}", line_acc)?;
        write!(self.stdout, "{}", style::Reset)?;
        for i in 0..self.path_surface.row_end - 1 {
            write!(
                self.stdout,
                "{}{}",
                cursor::Goto(1, self.path_surface.row_start + i),
                clear::CurrentLine,
            )?;
        }
        for (i, path) in self.filtered_paths.iter().enumerate() {
            let line = self.path_surface.row_end - (i as u16);
            if line < self.path_surface.row_start {
                break;
            }

            write!(
                self.stdout,
                "{}{}{}{}",
                cursor::Goto(1, line),
                clear::CurrentLine,
                path.to_str()
                    .ok_or(Error::PathUnicodeError(path.to_path_buf()))?,
                style::Reset,
            )?;
        }
        write!(self.stdout, "{}", cursor::Show)?;
        self.stdout.flush()?;
        Ok(())
    }

    pub fn register_help(
        &mut self,
        key: Key,
        help: impl Into<String>,
    ) -> Result<(), Error> {
        let key = match key {
            Key::Backspace => '\u{232B}'.to_string(),
            Key::Left => '\u{2190}'.to_string(),
            Key::Right => '\u{2192}'.to_string(),
            Key::Up => '\u{2191}'.to_string(),
            Key::Down => '\u{2193}'.to_string(),
            Key::Home => '\u{2912}'.to_string(),
            Key::End => '\u{2913}'.to_string(),
            Key::PageUp => '\u{21DE}'.to_string(),
            Key::PageDown => '\u{21DF}'.to_string(),
            Key::BackTab => '\u{21E4}'.to_string(),
            Key::Delete => '\u{2326}'.to_string(),
            Key::Insert => '\u{2324}'.to_string(),
            Key::F(n) => format!("F{n}"),
            Key::Char('\n') => '\u{21B2}'.to_string(),
            Key::Char('\t') => '\u{21E5}'.to_string(),
            Key::Char(ch) => ch.to_string(),
            Key::Alt(ch) => format!("alt + {}", ch),
            Key::Ctrl(ch) => format!("ctrl + {}", ch),
            Key::Esc => '\u{238B}'.to_string(),
            _ => '?'.to_string(),
        };
        let help = help.into();

        self.help.push(format!("{} \u{25BA} {}", key, help));

        let mut str = "".to_string();
        for help in &self.help {
            str = format!("{}{}    ", str, help);
        }

        let mut fill_width: String = "".to_string();
        for _ in 0..self.width {
            fill_width += " ";
        }
        write!(
            self.stdout,
            "{}{}{}{}{}{}",
            cursor::Hide,
            color::Bg(color::Black),
            cursor::Goto(1, 1),
            fill_width,
            cursor::Goto(1, 1),
            str,
        )?;

        self.stdout.flush()?;
        Ok(())
    }

    fn before_next_iter(&mut self) -> Result<(), Error> {
        write!(
            self.stdout,
            "{}{}> {}{}",
            style::Reset,
            cursor::Goto(1, self.height),
            self.input,
            clear::AfterCursor,
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    pub fn push(&mut self, ch: char) {
        self.input.push(ch);
    }
    pub fn pop(&mut self) -> Option<char> {
        self.input.pop()
    }
    pub fn get_input(&self) -> &str {
        &self.input
    }
    pub fn get_selected(&self) -> Option<PathBuf> {
        self.filtered_paths.get(0).cloned()
    }
}
impl<R: Iterator<Item = std::io::Result<Key>>, W: Write> Iterator
    for Window<R, W>
{
    type Item = Result<Key, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(err) = self.before_next_iter() {
            return Some(Err(err));
        }
        self.stdin
            .next()
            .map(|val| val.map_err(|err| Error::from(err)))
    }
}
impl<R: Read, W: Write> Window<termion::input::Keys<R>, W> {
    pub fn init(
        stdin: R,
        mut stdout: W,
        paths: Vec<PathBuf>,
    ) -> Result<Self, Error> {
        let (width, height) = terminal_size()?;
        info!("window created");
        write!(
            stdout,
            "{}{}{}",
            clear::All,
            style::Reset,
            cursor::Goto(1, 1),
        )?;
        stdout.flush()?;
        Ok(Self {
            stdin: stdin.keys(),
            stdout,
            path_surface: Surface::new(1, 2, width, height - 2),
            width,
            height,
            help: Vec::new(),
            input: String::new(),
            paths: paths.clone(),
            filtered_paths: paths,
        })
    }
}

impl<R, W: Write> Drop for Window<R, W> {
    fn drop(&mut self) {
        info!("window dropped");
        write!(
            self.stdout,
            "{}{}{}",
            clear::All,
            style::Reset,
            cursor::Goto(1, 1)
        )
        .unwrap();
    }
}

struct Surface {
    row_start: u16,
    col_start: u16,
    row_end: u16,
    col_end: u16,
}
impl Surface {
    fn new(col_start: u16, row_start: u16, col_end: u16, row_end: u16) -> Self {
        Self {
            col_start,
            row_start,
            col_end,
            row_end,
        }
    }
}
