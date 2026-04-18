mod ast;
mod lexer;
mod error;

use chumsky::Parser;
use lang::parser::parser;
use lang::lexer::lex;
use std::fs;
use miette::SourceSpan;
use chumsky::input::Input;
use chumsky::span::Span;

fn main() {
    let source = fs::read_to_string("examples/test_managed.sr").expect("test_managed.sr not found");
    let tokens = lex(&source).unwrap();
    let end_span = chumsky::span::SimpleSpan::new((), source.len()..source.len());
    let input = tokens.as_slice().split_token_span(end_span);
    let ast = parser().parse(input).into_result().unwrap();
    println!("{:#?}", ast);
}
