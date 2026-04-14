use lang::lexer::lex;
use std::fs;

fn main() {
    let source = fs::read_to_string("main.sr").unwrap();
    let tokens = lex(&source);
    for (i, t) in tokens.iter().enumerate() {
        println!("{}: {:?}", i, t);
    }
}
