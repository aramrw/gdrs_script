use miette::SourceSpan;
use chumsky::span::SimpleSpan;
use crate::error::LexError;

use std::fmt;

pub type Span = SimpleSpan<usize>;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    // Keywords
    Fn, Var, Mut, Const, Box, Rc, Arc, If, Else, Print, Log, LogLn, Obj, Impl, Enum, Match, While, Return, Extern, SelfKw, Use, ResultKw, ErrorKw, Rust, Async, Await, Dependency, Any, As, Break, Loop,
    // Types
    I32, I64, F32, F64, Bool, Str, StringKw, File,
    // Literals
    Int(i32), Int64(i64), Float(f32), Float64(f64), Boolean(bool), String(String), Ident(String),
    // Symbols
    Plus, Minus, Star, Div, Eq, DoubleEq, Amp, Colon, Arrow, Dot, Gt, Lt, QuestionMark, Bang,
    ParenOpen, ParenClose, BraceOpen, BraceClose, BracketOpen, BracketClose,
    Comma, Semicolon, DoubleColon, FatArrow, Tilde,
    // Significant Whitespace
    Indent, Dedent,
    Attribute(String),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Token {
    pub fn to_string(&self) -> String {
        match self {
            Token::Fn => "fn".to_string(),
            Token::Var => "var".to_string(),
            Token::Mut => "mut".to_string(),
            Token::Const => "const".to_string(),
            Token::Box => "box".to_string(),
            Token::Rc => "rc".to_string(),
            Token::Arc => "arc".to_string(),
            Token::If => "if".to_string(),
            Token::Else => "else".to_string(),
            Token::Print => "print".to_string(),
            Token::Log => "log".to_string(),
            Token::LogLn => "logln".to_string(),
            Token::Obj => "obj".to_string(),
            Token::Impl => "impl".to_string(),
            Token::Enum => "enum".to_string(),
            Token::Match => "match".to_string(),
            Token::While => "while".to_string(),
            Token::Return => "return".to_string(),
            Token::Extern => "extern".to_string(),
            Token::SelfKw => "self".to_string(),
            Token::Use => "use".to_string(),
            Token::ResultKw => "Result".to_string(),
            Token::ErrorKw => "Error".to_string(),
            Token::Rust => "rust".to_string(),
            Token::Async => "async".to_string(),
            Token::Await => "await".to_string(),
            Token::Dependency => "dependency".to_string(),
            Token::Any => "any".to_string(),
            Token::As => "as".to_string(),
            Token::Break => "break".to_string(),
            Token::Loop => "loop".to_string(),
            Token::I32 => "i32".to_string(),
            Token::I64 => "i64".to_string(),
            Token::F32 => "f32".to_string(),
            Token::F64 => "f64".to_string(),
            Token::Bool => "bool".to_string(),
            Token::Str => "str".to_string(),
            Token::StringKw => "String".to_string(),
            Token::File => "File".to_string(),
            Token::Int(i) => i.to_string(),
            Token::Int64(i) => i.to_string(),
            Token::Float(f) => f.to_string(),
            Token::Float64(f) => f.to_string(),
            Token::Boolean(b) => b.to_string(),
            Token::String(s) => format!("\"{}\"", s),
            Token::Ident(s) => s.clone(),
            Token::Plus => "+".to_string(),
            Token::Minus => "-".to_string(),
            Token::Star => "*".to_string(),
            Token::Div => "/".to_string(),
            Token::Eq => "=".to_string(),
            Token::DoubleEq => "==".to_string(),
            Token::Amp => "&".to_string(),
            Token::Colon => ":".to_string(),
            Token::Arrow => "->".to_string(),
            Token::Dot => ".".to_string(),
            Token::Gt => ">".to_string(),
            Token::Lt => "<".to_string(),
            Token::QuestionMark => "?".to_string(),
            Token::Bang => "!".to_string(),
            Token::ParenOpen => "(".to_string(),
            Token::ParenClose => ")".to_string(),
            Token::BraceOpen => "{".to_string(),
            Token::BraceClose => "}".to_string(),
            Token::BracketOpen => "[".to_string(),
            Token::BracketClose => "]".to_string(),
            Token::Comma => ",".to_string(),
            Token::Semicolon => ";".to_string(),
            Token::DoubleColon => "::".to_string(),
            Token::FatArrow => "=>".to_string(),
            Token::Tilde => "~".to_string(),
            Token::Indent => "<indent>".to_string(),
            Token::Dedent => "".to_string(),
            Token::Attribute(s) => format!("#[{}]", s),
        }
    }
}

pub fn lex(source: &str) -> Result<Vec<(Token, Span)>, LexError> {
    let mut tokens = Vec::new();
    let mut indents = vec![0];
    let mut nest_level = 0;
    let mut offset = 0;
    
    for line in source.lines() {
        let line_len = line.len();
        let trimmed = line.trim();
        let line_start_offset = offset;
        
        if trimmed.is_empty() || trimmed.starts_with("//") { 
            offset += line_len + 1; // +1 for newline
            continue; 
        }

        let leading_whitespace = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect::<String>();
        let whitespace_len = leading_whitespace.len();

        // Only handle significant indentation if we are NOT inside brackets
        if nest_level == 0 {
            let mut indent = 0;
            for c in leading_whitespace.chars() {
                if c == '\t' { indent += 4; }
                else { indent += 1; }
            }
            let last_indent = *indents.last().unwrap();

            if indent > last_indent {
                indents.push(indent);
                tokens.push((Token::Indent, SimpleSpan::from(line_start_offset..line_start_offset + whitespace_len)));
            } else if indent < last_indent {
                while indent < *indents.last().unwrap() {
                    indents.pop();
                    tokens.push((Token::Dedent, SimpleSpan::from(line_start_offset..line_start_offset + whitespace_len)));
                }
                
                let new_last = *indents.last().unwrap();
                if indent != new_last {
                    return Err(LexError::InvalidIndentation {
                        span: SourceSpan::new(line_start_offset.into(), whitespace_len.into()),
                    });
                }
            }
        }

        let trimmed_start_offset = line_start_offset + whitespace_len;
        let mut chars = trimmed.chars().enumerate().peekable();
        
        while let Some((i, c)) = chars.next() {
            let current_pos = trimmed_start_offset + i;
            let span = |len: usize| SimpleSpan::from(current_pos..current_pos + len);

            match c {
                ' ' | '\t' | '\r' => continue,
                '/' if chars.peek().map(|&(_, c)| c) == Some('/') => break,
                '#' => {
                    if chars.peek().map(|&(_, c)| c) == Some('[') {
                        chars.next();
                        let mut s = String::new();
                        let mut depth = 1;
                        let start = current_pos;
                        while let Some((_, nc)) = chars.next() {
                            if nc == '[' { depth += 1; }
                            if nc == ']' { 
                                depth -= 1; 
                                if depth == 0 { break; }
                            }
                            s.push(nc);
                        }
                        let end = trimmed_start_offset + chars.peek().map_or(trimmed.len(), |&(i, _)| i);
                        tokens.push((Token::Attribute(s), SimpleSpan::from(start..end)));
                    } else {
                        break;
                    }
                }
                '(' => { tokens.push((Token::ParenOpen, span(1))); nest_level += 1; }
                ')' => { tokens.push((Token::ParenClose, span(1))); nest_level -= 1; }
                '{' => { tokens.push((Token::BraceOpen, span(1))); nest_level += 1; }
                '}' => { tokens.push((Token::BraceClose, span(1))); nest_level -= 1; }
                '[' => { tokens.push((Token::BracketOpen, span(1))); nest_level += 1; }
                ']' => { tokens.push((Token::BracketClose, span(1))); nest_level -= 1; }
                ',' => tokens.push((Token::Comma, span(1))),
                ';' => tokens.push((Token::Semicolon, span(1))),
                '+' => tokens.push((Token::Plus, span(1))),
                '&' => tokens.push((Token::Amp, span(1))),
                '*' => tokens.push((Token::Star, span(1))),
                '/' => tokens.push((Token::Div, span(1))),
                '>' => tokens.push((Token::Gt, span(1))),
                '<' => tokens.push((Token::Lt, span(1))),
                '.' => tokens.push((Token::Dot, span(1))),
                '?' => tokens.push((Token::QuestionMark, span(1))),
                '!' => tokens.push((Token::Bang, span(1))),
                '~' => tokens.push((Token::Tilde, span(1))),
                '-' => {
                    if chars.peek().map(|&(_, c)| c) == Some('>') { 
                        chars.next(); 
                        tokens.push((Token::Arrow, span(2))); 
                    }
                    else { tokens.push((Token::Minus, span(1))); }
                }
                ':' => {
                    if chars.peek().map(|&(_, c)| c) == Some(':') { 
                        chars.next(); 
                        tokens.push((Token::DoubleColon, span(2))); 
                    }
                    else { tokens.push((Token::Colon, span(1))); }
                }
                '=' => {
                    if chars.peek().map(|&(_, c)| c) == Some('>') { 
                        chars.next(); 
                        tokens.push((Token::FatArrow, span(2))); 
                    }
                    else if chars.peek().map(|&(_, c)| c) == Some('=') { 
                        chars.next(); 
                        tokens.push((Token::DoubleEq, span(2))); 
                    }
                    else { tokens.push((Token::Eq, span(1))); }
                }
                '"' => {
                    let mut s = String::new();
                    let start = current_pos;
                    while let Some((_, nc)) = chars.next() {
                        if nc == '"' { break; }
                        if nc == '\\' {
                            match chars.next() {
                                Some((_, 'n')) => s.push('\n'),
                                Some((_, 'r')) => s.push('\r'),
                                Some((_, 't')) => s.push('\t'),
                                Some((_, '\\')) => s.push('\\'),
                                Some((_, '"')) => s.push('"'),
                                Some((_, other)) => { s.push('\\'); s.push(other); }
                                None => s.push('\\'),
                            }
                        } else {
                            s.push(nc);
                        }
                    }
                    let end = trimmed_start_offset + chars.peek().map_or(trimmed.len(), |&(i, _)| i);
                    tokens.push((Token::String(s), SimpleSpan::from(start..end)));
                }
                _ if c.is_ascii_digit() => {
                    let start = current_pos;
                    let mut s = c.to_string();
                    let mut is_float = false;
                    while let Some(&(_, nc)) = chars.peek() {
                        if nc.is_ascii_digit() { s.push(chars.next().unwrap().1); }
                        else if nc == '.' && !is_float { 
                            is_float = true; 
                            s.push(chars.next().unwrap().1); 
                        }
                        else { break; }
                    }
                    
                    let mut suffix = String::new();
                    while let Some(&(_, nc)) = chars.peek() {
                        if nc.is_ascii_alphanumeric() { suffix.push(chars.next().unwrap().1); }
                        else { break; }
                    }

                    let end = trimmed_start_offset + chars.peek().map_or(trimmed.len(), |&(i, _)| i);
                    let span = SimpleSpan::from(start..end);

                    match suffix.as_str() {
                        "f32" => tokens.push((Token::Float(s.parse().unwrap()), span)),
                        "f64" => tokens.push((Token::Float64(s.parse().unwrap()), span)),
                        "i64" => tokens.push((Token::Int64(s.parse().unwrap()), span)),
                        "i32" => tokens.push((Token::Int(s.parse().unwrap()), span)),
                        "" => {
                            if is_float { tokens.push((Token::Float(s.parse().unwrap()), span)); }
                            else { 
                                if let Ok(val) = s.parse::<i32>() {
                                    tokens.push((Token::Int(val), span));
                                } else if let Ok(val) = s.parse::<i64>() {
                                    tokens.push((Token::Int64(val), span));
                                } else {
                                    tokens.push((Token::Int(s.parse().unwrap()), span));
                                }
                            }
                        }
                        _ => {
                            if is_float { tokens.push((Token::Float(s.parse().unwrap()), span)); }
                            else { tokens.push((Token::Int(s.parse().unwrap()), span)); }
                        }
                    }
                }
                _ if c.is_ascii_alphabetic() || c == '_' => {
                    let start = current_pos;
                    let mut s = c.to_string();
                    while let Some(&(_, nc)) = chars.peek() {
                        if nc.is_ascii_alphanumeric() || nc == '_' { s.push(chars.next().unwrap().1); }
                        else { break; }
                    }
                    let end = trimmed_start_offset + chars.peek().map_or(trimmed.len(), |&(i, _)| i);
                    let span = SimpleSpan::from(start..end);

                    match s.as_str() {
                        "fn" => tokens.push((Token::Fn, span)),
                        "var" => tokens.push((Token::Var, span)),
                        "mut" => tokens.push((Token::Mut, span)),
                        "const" => tokens.push((Token::Const, span)),
                        "box" => tokens.push((Token::Box, span)),
                        "rc" => tokens.push((Token::Rc, span)),
                        "arc" => tokens.push((Token::Arc, span)),
                        "if" => tokens.push((Token::If, span)),
                        "else" => tokens.push((Token::Else, span)),
                        "print" => tokens.push((Token::Print, span)),
                        "log" => tokens.push((Token::Log, span)),
                        "logln" => tokens.push((Token::LogLn, span)),
                        "obj" => tokens.push((Token::Obj, span)),
                        "impl" => tokens.push((Token::Impl, span)),
                        "enum" => tokens.push((Token::Enum, span)),
                        "match" => tokens.push((Token::Match, span)),
                        "while" => tokens.push((Token::While, span)),
                        "return" => tokens.push((Token::Return, span)),
                        "extern" => tokens.push((Token::Extern, span)),
                        "use" => tokens.push((Token::Use, span)),
                        "rust" => tokens.push((Token::Rust, span)),
                        "as" => tokens.push((Token::As, span)),
                        "async" => tokens.push((Token::Async, span)),
                        "await" => tokens.push((Token::Await, span)),
                        "dependency" => tokens.push((Token::Dependency, span)),
                        "self" => tokens.push((Token::SelfKw, span)),
                        "break" => tokens.push((Token::Break, span)),
                        "loop" => tokens.push((Token::Loop, span)),
                        "result" => tokens.push((Token::ResultKw, span)),
                        "error" => tokens.push((Token::ErrorKw, span)),
                        "any" => tokens.push((Token::Any, span)),
                        "i32" => tokens.push((Token::I32, span)),
                        "i64" => tokens.push((Token::I64, span)),
                        "f32" => tokens.push((Token::F32, span)),
                        "f64" => tokens.push((Token::F64, span)),
                        "bool" => tokens.push((Token::Bool, span)),
                        "str" => tokens.push((Token::Str, span)),
                        "string" => tokens.push((Token::StringKw, span)),
                        "file" => tokens.push((Token::File, span)),
                        "true" => tokens.push((Token::Boolean(true), span)),
                        "false" => tokens.push((Token::Boolean(false), span)),
                        _ => tokens.push((Token::Ident(s), span)),
                    }
                }
                _ => {
                    return Err(LexError::UnexpectedCharacter {
                        character: c,
                        span: SourceSpan::new(current_pos.into(), 1),
                    });
                }
            }
        }
        
        if nest_level == 0 && !tokens.is_empty() {
            let (last, last_span) = tokens.last().unwrap();
            match last {
                Token::Colon | Token::Comma | Token::Semicolon | Token::Indent | Token::Dedent | Token::ParenOpen | Token::BracketOpen | Token::BraceOpen | Token::Attribute(_) => {},
                _ => { tokens.push((Token::Semicolon, SimpleSpan::from(last_span.end..last_span.end))); }
            }
        }

        offset += line_len + 1;
    }

    let final_pos = offset;
    while indents.len() > 1 { 
        indents.pop(); 
        tokens.push((Token::Dedent, SimpleSpan::from(final_pos..final_pos))); 
    }
    
    Ok(tokens)
}
