use std::{collections::hash_map::DefaultHasher, hash::Hasher};

use termion::color;

const SPINNER: [char; 3] = ['-', '/', '\\'];

pub struct Spinner {
    pos: usize,
}

impl Spinner {
    pub fn new() -> Spinner {
        Spinner { pos: 0 }
    }

    pub fn next(&mut self) -> char {
        let ch = SPINNER[self.pos];
        self.pos = (self.pos + 1) % SPINNER.len();
        ch
    }

    pub fn peek(&self) -> char {
        SPINNER[self.pos]
    }
}

pub fn ucolor<T: std::hash::Hash>(value: &T) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    let n: u64 = hasher.finish();
    get_ansi_color(n)
}

// FIXME: Thanks termion
fn get_color(n: u64) -> String {
    match n % 10 {
        0 => color::Green.fg_str(),
        1 => color::Blue.fg_str(),
        2 => color::Yellow.fg_str(),
        3 => color::Magenta.fg_str(),
        4 => color::Cyan.fg_str(),
        5 => color::Red.fg_str(),
        6 => color::LightBlack.fg_str(),
        7 => color::LightRed.fg_str(),
        8 => color::LightCyan.fg_str(),
        9 => color::LightMagenta.fg_str(),
        _ => unreachable!(),
    }
    .to_string()
}

fn get_ansi_color(n: u64) -> String {
    color::AnsiValue((n % 256) as u8).fg_string()
}
