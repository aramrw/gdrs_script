mod ast;
mod lexer;

use lexer::lex;
use std::fs;

fn main() {
    let source = fs::read_to_string("test_macroquad.sr").expect("test_macroquad.sr not found");
    let tokens = lex(&source);
    for (i, t) in tokens.iter().enumerate() {
        println!("{}: {:?}", i, t);
    }
}
