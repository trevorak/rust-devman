use std::io::{self, Write};

pub fn must_get_string(msg: &str) -> String {
    loop {
        print!("{}", msg);

        let input = io_read_string();

        if !input.is_empty() {
            return input;
        }
    }
}

pub fn get_string(msg: &str) -> String {
    print!("{}", msg.to_string());

    io_read_string()
}

fn io_read_string() -> String {
    io::stdout().flush().unwrap();

    let mut input = String::new();

    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");

    input.trim().to_string()
}