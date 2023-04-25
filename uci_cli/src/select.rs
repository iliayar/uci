use std::io::{stdin, stdout, BufRead, Write};

use termion::{clear, cursor, event::Key, input::TermRead, raw::IntoRawMode, style};

use crate::execute::ExecuteError;

pub trait SelectOption {
    type Data: std::fmt::Display;

    fn show(&self, out: &mut impl Write);
    fn data(self) -> Self::Data;
    fn data_name(&self) -> &str;
}

pub fn prompt<T: SelectOption>(options: impl Iterator<Item = T>) -> Result<T::Data, ExecuteError> {
    let mut options: Vec<T> = options.collect();
    assert!(options.len() > 0, "No options to select from");

    println!(
        "Select {}{}{}: ",
        style::Bold,
        options[0].data_name(),
        style::NoBold
    );

    let options_lines = get_lines(&options, 3);

    let mut selection = 0usize;
    let total_lines: usize = options_lines.iter().map(|lines| lines.len()).sum();

    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode().unwrap();

    let mut print_options = |selection: usize, init: bool| {
        if !init {
            write!(stdout, "{}", cursor::Up(total_lines as u16)).ok();
        }

        for (i, option) in options.iter().enumerate() {
            write!(stdout, "{}", clear::CurrentLine).ok();
            if i == selection {
                write!(stdout, "{}", style::Invert).ok();
            }

            write!(stdout, "[{}]{} ", i + 1, style::Reset).ok();

            for line in options_lines[i].iter() {
                write!(stdout, "{}\n\r", line).ok();
            }
        }

        stdout.flush().ok();
    };

    let inc = |selection: usize| {
        if selection < options_lines.len() - 1 {
            selection + 1
        } else {
            selection
        }
    };

    let dec = |selection: usize| {
        if selection > 0 {
            selection - 1
        } else {
            selection
        }
    };

    print_options(selection, true);

    for c in stdin.keys() {
        match c.unwrap() {
            Key::Char('j') => selection = inc(selection),
            Key::Char('k') => selection = dec(selection),
            Key::Ctrl('n') => selection = inc(selection),
            Key::Ctrl('p') => selection = dec(selection),
            Key::Down => selection = inc(selection),
            Key::Up => selection = dec(selection),
            Key::Char('\n') => {
                break;
            }
            Key::Ctrl('c') => {
		write!(stdout, "{}", cursor::Show).ok();
		stdout.flush().ok();
                return Err(ExecuteError::Interrupted);
            }

            Key::Char(c) if ('1'..'9').contains(&c) => {
                selection = c.to_string().parse::<usize>().unwrap() - 1;
                break;
            }
            _ => {}
        }

        print_options(selection, false);
    }

    drop(print_options);

    write!(
        stdout,
        "{}{}",
        cursor::Up(total_lines as u16 + 1),
        clear::AfterCursor
    )
    .ok();

    drop(stdout);

    let selection = options.swap_remove(selection);
    print!(
        "Selected {}{}{}: ",
        style::Bold,
        selection.data_name(),
        style::NoBold
    );

    let data = selection.data();
    println!("{}", data);
    Ok(data)
}

fn get_lines<T: SelectOption>(options: &Vec<T>, additional_padding: usize) -> Vec<Vec<String>> {
    options
        .iter()
        .enumerate()
        .map(|(i, option)| {
            let padding = " ".repeat(i.to_string().len() + additional_padding);

            let mut output = Vec::new();
            option.show(&mut output);

            output
                .lines()
                .enumerate()
                .map(|(j, line)| {
                    // FIXME: This unwrap
                    if j == 0 {
                        line.unwrap()
                    } else {
                        format!("{}{}", padding, line.unwrap())
                    }
                })
                .collect()
        })
        .collect()
}

pub fn prompt_many<T: SelectOption>(
    options: impl Iterator<Item = T>,
) -> Result<Vec<T::Data>, ExecuteError> {
    let options: Vec<T> = options.collect();

    if options.len() == 0 {
        return Ok(Vec::new());
    }

    println!(
        "Select {}{}s{}: ",
        style::Bold,
        options[0].data_name(),
        style::NoBold
    );

    let options_lines = get_lines(&options, 4);

    let mut selection = 0usize;
    let mut selected: Vec<bool> = vec![true; options.len()];
    let total_lines: usize = options_lines.iter().map(|lines| lines.len()).sum::<usize>() + 1;

    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode().unwrap();

    write!(stdout, "{}", cursor::Hide).ok();

    let mut print_options = |selected: &Vec<bool>, selection: usize, init: bool| {
        if !init {
            write!(stdout, "{}", cursor::Up(total_lines as u16)).ok();
        }

        write!(stdout, "{}", clear::CurrentLine).ok();
        if selection == 0 {
            write!(stdout, ">").ok();
        } else {
            write!(stdout, " ").ok();
        }

        if selected.iter().all(|v| *v) {
            write!(stdout, "{}", style::Invert).ok();
        }
        write!(stdout, "[0]{} All\n\r", style::Reset).ok();

        for (i, option) in options.iter().enumerate() {
            write!(stdout, "{}", clear::CurrentLine).ok();

            if i + 1 == selection {
                write!(stdout, ">").ok();
            } else {
                write!(stdout, " ").ok();
            }

            if selected[i] {
                write!(stdout, "{}", style::Invert).ok();
            }

            write!(stdout, "[{}]{} ", i + 1, style::Reset).ok();

            for line in options_lines[i].iter() {
                write!(stdout, "{}\n\r", line).ok();
            }
        }

        stdout.flush().ok();
    };

    let inc = |selection: usize| {
        if selection < options_lines.len() {
            selection + 1
        } else {
            selection
        }
    };

    let dec = |selection: usize| {
        if selection > 0 {
            selection - 1
        } else {
            selection
        }
    };

    print_options(&selected, selection, true);

    for c in stdin.keys() {
        match c.unwrap() {
            Key::Char('j') => selection = inc(selection),
            Key::Char('k') => selection = dec(selection),
            Key::Ctrl('n') => selection = inc(selection),
            Key::Ctrl('p') => selection = dec(selection),
            Key::Down => selection = inc(selection),
            Key::Up => selection = dec(selection),
            Key::Char('\n') => {
                break;
            }
            Key::Ctrl('c') => {
		write!(stdout, "{}", cursor::Show).ok();
		stdout.flush().ok();
                return Err(ExecuteError::Interrupted);
            }

            Key::Char(c) if ('1'..'9').contains(&c) => {
                let i = c.to_string().parse::<usize>().unwrap() - 1;
                selected[i] = !selected[i];
            }

            Key::Char('0') => {
                let new_value = !selected.iter().all(|v| *v);
                selected = vec![new_value; options.len()];
            }

            Key::Char(' ') => {
                if selection == 0 {
                    let new_value = !selected.iter().all(|v| *v);
                    selected = vec![new_value; options.len()];
                } else {
                    selected[selection - 1] = !selected[selection - 1];
                }
            }
            _ => {}
        }

        print_options(&selected, selection, false);
    }

    drop(print_options);

    write!(
        stdout,
        "{}{}",
        cursor::Up(total_lines as u16 + 1),
        clear::AfterCursor
    )
    .ok();
    write!(stdout, "{}", cursor::Show).ok();

    stdout.flush().ok();

    drop(stdout);

    println!(
        "Selected {}{}s{}: ",
        style::Bold,
        options[0].data_name(),
        style::NoBold
    );

    let data: Vec<T::Data> = options
        .into_iter()
        .zip(selected.into_iter())
        .filter_map(|(opt, v)| if v { Some(opt.data()) } else { None })
        .collect();

    for d in data.iter() {
        println!("- {}", d);
    }

    Ok(data)
}
