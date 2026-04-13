use crate::ast::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    // Keywords
    Fn, Var, Mut, Const, Box, If, Else, Print, Obj, Impl, Enum, Match, While, Return, Extern, SelfKw, Use,
    // Types
    I32, F32, Bool, Str, StringKw, File,
    // Literals
    Int(i32), Float(f32), Boolean(bool), String(String), Ident(String),
    // Symbols
    Plus, Minus, Star, Div, Eq, Colon, Arrow, Dot, Gt, Lt,
    ParenOpen, ParenClose, BraceOpen, BraceClose, BracketOpen, BracketClose,
    Comma, Semicolon, DoubleColon, FatArrow,
    // Significant Whitespace
    Indent, Dedent,
}

pub fn lex(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut indents = vec![0];
    let mut nest_level = 0;
    
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") { continue; }

        // Only handle significant indentation if we are NOT inside brackets
        if nest_level == 0 {
            let mut indent = 0;
            for c in line.chars().take_while(|c| *c == ' ' || *c == '\t') {
                if c == '\t' { indent += 4; }
                else { indent += 1; }
            }
            let last_indent = *indents.last().unwrap();

            if indent > last_indent {
                indents.push(indent);
                tokens.push(Token::Indent);
            } else {
                while indent < *indents.last().unwrap() {
                    indents.pop();
                    tokens.push(Token::Dedent);
                }
            }
        }

        let mut chars = trimmed.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                ' ' | '\t' | '\r' => continue,
                '(' => { tokens.push(Token::ParenOpen); nest_level += 1; }
                ')' => { tokens.push(Token::ParenClose); nest_level -= 1; }
                '{' => { tokens.push(Token::BraceOpen); nest_level += 1; }
                '}' => { tokens.push(Token::BraceClose); nest_level -= 1; }
                '[' => { tokens.push(Token::BracketOpen); nest_level += 1; }
                ']' => { tokens.push(Token::BracketClose); nest_level -= 1; }
                ',' => tokens.push(Token::Comma),
                ';' => tokens.push(Token::Semicolon),
                '+' => tokens.push(Token::Plus),
                '*' => tokens.push(Token::Star),
                '/' => tokens.push(Token::Div),
                '>' => tokens.push(Token::Gt),
                '<' => tokens.push(Token::Lt),
                '.' => tokens.push(Token::Dot),
                '-' => {
                    if chars.peek() == Some(&'>') { chars.next(); tokens.push(Token::Arrow); }
                    else { tokens.push(Token::Minus); }
                }
                ':' => {
                    if chars.peek() == Some(&':') { chars.next(); tokens.push(Token::DoubleColon); }
                    else { tokens.push(Token::Colon); }
                }
                '=' => {
                    if chars.peek() == Some(&'>') { chars.next(); tokens.push(Token::FatArrow); }
                    else { tokens.push(Token::Eq); }
                }
                '"' => {
                    let mut s = String::new();
                    while let Some(&nc) = chars.peek() {
                        if nc == '"' { chars.next(); break; }
                        s.push(chars.next().unwrap());
                    }
                    tokens.push(Token::String(s));
                }
                _ if c.is_ascii_digit() => {
                    let mut s = c.to_string();
                    let mut is_float = false;
                    while let Some(&nc) = chars.peek() {
                        if nc.is_ascii_digit() { s.push(chars.next().unwrap()); }
                        else if nc == '.' { is_float = true; s.push(chars.next().unwrap()); }
                        else { break; }
                    }
                    if is_float { tokens.push(Token::Float(s.parse().unwrap())); }
                    else { tokens.push(Token::Int(s.parse().unwrap())); }
                }
                _ if c.is_ascii_alphabetic() || c == '_' => {
                    let mut s = c.to_string();
                    while let Some(&nc) = chars.peek() {
                        if nc.is_ascii_alphanumeric() || nc == '_' { s.push(chars.next().unwrap()); }
                        else { break; }
                    }
                    match s.as_str() {
                        "fn" => tokens.push(Token::Fn),
                        "var" => tokens.push(Token::Var),
                        "mut" => tokens.push(Token::Mut),
                        "const" => tokens.push(Token::Const),
                        "box" => tokens.push(Token::Box),
                        "if" => tokens.push(Token::If),
                        "else" => tokens.push(Token::Else),
                        "print" => tokens.push(Token::Print),
                        "obj" => tokens.push(Token::Obj),
                        "impl" => tokens.push(Token::Impl),
                        "enum" => tokens.push(Token::Enum),
                        "match" => tokens.push(Token::Match),
                        "while" => tokens.push(Token::While),
                        "return" => tokens.push(Token::Return),
                        "extern" => tokens.push(Token::Extern),
                        "use" => tokens.push(Token::Use),
                        "self" => tokens.push(Token::SelfKw),
                        "i32" => tokens.push(Token::I32),
                        "f32" => tokens.push(Token::F32),
                        "bool" => tokens.push(Token::Bool),
                        "str" => tokens.push(Token::Str),
                        "string" => tokens.push(Token::StringKw),
                        "file" => tokens.push(Token::File),
                        "true" => tokens.push(Token::Boolean(true)),
                        "false" => tokens.push(Token::Boolean(false)),
                        _ => tokens.push(Token::Ident(s)),
                    }
                }
                _ => {}
            }
        }
    }

    while indents.len() > 1 { indents.pop(); tokens.push(Token::Dedent); }
    tokens
}
