use std::io::{self, Read};
use analyzer::formatter;

// Assuming formatter is public in the lib or I need to move it
fn main() -> io::Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    let formatted = analyzer::formatter::format_code(&buffer);
    print!("{}", formatted);
    Ok(())
}
