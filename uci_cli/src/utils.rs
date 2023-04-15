
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
}
