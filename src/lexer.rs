#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    // Keywords
    Fn, Var, Mut, Const, Box, If, Else, Print, Log, LogLn, Obj, Impl, Enum, Match, While, Return, Extern, SelfKw, Use, ResultKw, ErrorKw, Rust, Async, Await, Dependency, Any, As,
    // Types
    I32, I64, F32, F64, Bool, Str, StringKw, File,
    // Literals
    Int(i32), Int64(i64), Float(f32), Float64(f64), Boolean(bool), String(String), Ident(String),
    // Symbols
    Plus, Minus, Star, Div, Eq, DoubleEq, Amp, Colon, Arrow, Dot, Gt, Lt, QuestionMark, Bang,
    ParenOpen, ParenClose, BraceOpen, BraceClose, BracketOpen, BracketClose,
    Comma, Semicolon, DoubleColon, FatArrow,
    // Significant Whitespace
    Indent, Dedent,
    Attribute(String),
}

impl Token {
    pub fn to_string(&self) -> String {
        match self {
            Token::Fn => "fn".to_string(),
            Token::Var => "var".to_string(),
            Token::Mut => "mut".to_string(),
            Token::Const => "const".to_string(),
            Token::Box => "Box".to_string(),
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
            Token::Indent => "".to_string(),
            Token::Dedent => "".to_string(),
            Token::Attribute(s) => format!("#[{}]", s),
        }
    }
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
            } else if indent < last_indent {
                while indent < *indents.last().unwrap() {
                    indents.pop();
                    tokens.push(Token::Dedent);
                }
                // If we dedented to a level that is still NOT our target indent,
                // it's an error in most languages, but we can try to be helpful.
                // Re-check last_indent.
                let new_last = *indents.last().unwrap();
                if indent > new_last {
                    indents.push(indent);
                    tokens.push(Token::Indent);
                }
            }
        }

        let mut chars = trimmed.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                ' ' | '\t' | '\r' => continue,
                '/' if chars.peek() == Some(&'/') => break,
                '#' => {
                    if chars.peek() == Some(&'[') {
                        chars.next();
                        let mut s = String::new();
                        let mut depth = 1;
                        while let Some(nc) = chars.next() {
                            if nc == '[' { depth += 1; }
                            if nc == ']' { 
                                depth -= 1; 
                                if depth == 0 { break; }
                            }
                            s.push(nc);
                        }
                        tokens.push(Token::Attribute(s));
                    } else {
                        break;
                    }
                }
                '(' => { tokens.push(Token::ParenOpen); nest_level += 1; }
                ')' => { tokens.push(Token::ParenClose); nest_level -= 1; }
                '{' => { tokens.push(Token::BraceOpen); nest_level += 1; }
                '}' => { tokens.push(Token::BraceClose); nest_level -= 1; }
                '[' => { tokens.push(Token::BracketOpen); nest_level += 1; }
                ']' => { tokens.push(Token::BracketClose); nest_level -= 1; }
                ',' => tokens.push(Token::Comma),
                ';' => tokens.push(Token::Semicolon),
                '+' => tokens.push(Token::Plus),
                '&' => tokens.push(Token::Amp),
                '*' => tokens.push(Token::Star),
                '/' => tokens.push(Token::Div),
                '>' => tokens.push(Token::Gt),
                '<' => tokens.push(Token::Lt),
                '.' => tokens.push(Token::Dot),
                '?' => tokens.push(Token::QuestionMark),
                '!' => tokens.push(Token::Bang),
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
                    else if chars.peek() == Some(&'=') { chars.next(); tokens.push(Token::DoubleEq); }
                    else { tokens.push(Token::Eq); }
                }
                '"' => {
                    let mut s = String :: new () ;
                    while let Some (nc) = chars . next () {
                        if nc == '"' { break ; }
                        if nc == '\\' {
                            match chars . next () {
                                Some ('n') => s . push ('\n') ,
                                Some ('r') => s . push ('\r') ,
                                Some ('t') => s . push ('\t') ,
                                Some ('\\') => s . push ('\\') ,
                                Some ('"') => s . push ('"') ,
                                Some (other) => { s . push ('\\') ; s . push (other) ; }
                                None => s . push ('\\') ,
                            }
                        } else {
                            s . push (nc) ;
                        }
                    }
                    tokens . push (Token :: String (s)) ;
                }
                _ if c.is_ascii_digit() => {
                    let mut s = c.to_string();
                    let mut is_float = false;
                    while let Some(&nc) = chars.peek() {
                        if nc.is_ascii_digit() { s.push(chars.next().unwrap()); }
                        else if nc == '.' && !is_float { 
                            is_float = true; 
                            s.push(chars.next().unwrap()); 
                        }
                        else { break; }
                    }
                    
                    // Check for suffixes
                    let mut suffix = String::new();
                    while let Some(&nc) = chars.peek() {
                        if nc.is_ascii_alphanumeric() { suffix.push(chars.next().unwrap()); }
                        else { break; }
                    }

                    match suffix.as_str() {
                        "f32" => tokens.push(Token::Float(s.parse().unwrap())),
                        "f64" => tokens.push(Token::Float64(s.parse().unwrap())),
                        "i64" => tokens.push(Token::Int64(s.parse().unwrap())),
                        "i32" => tokens.push(Token::Int(s.parse().unwrap())),
                        "" => {
                            if is_float { tokens.push(Token::Float(s.parse().unwrap())); }
                            else { 
                                // Try i32 first, then i64
                                if let Ok(val) = s.parse::<i32>() {
                                    tokens.push(Token::Int(val));
                                } else if let Ok(val) = s.parse::<i64>() {
                                    tokens.push(Token::Int64(val));
                                } else {
                                    // Fallback to float if it's too huge for i64? 
                                    // Or just panic/error. For now unwrap.
                                    tokens.push(Token::Int(s.parse().unwrap()));
                                }
                            }
                        }
                        _ => {
                            // If suffix is unknown, we might have eaten part of next identifier
                            // This is a bit risky but we can try to backtrack or just assume it's wrong for now.
                            // In this simple lexer, we'll just push what we have.
                            if is_float { tokens.push(Token::Float(s.parse().unwrap())); }
                            else { tokens.push(Token::Int(s.parse().unwrap())); }
                        }
                    }
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
                        "log" => tokens.push(Token::Log),
                        "logln" => tokens.push(Token::LogLn),
                        "obj" => tokens.push(Token::Obj),
                        "impl" => tokens.push(Token::Impl),
                        "enum" => tokens.push(Token::Enum),
                        "match" => tokens.push(Token::Match),
                        "while" => tokens.push(Token::While),
                        "return" => tokens.push(Token::Return),
                        "extern" => tokens.push(Token::Extern),
                        "use" => tokens.push(Token::Use),
                        "rust" => tokens.push(Token::Rust),
                        "as" => tokens.push(Token::As),
                        "async" => tokens.push(Token::Async),
                        "await" => tokens.push(Token::Await),
                        "dependency" => tokens.push(Token::Dependency),
                        "self" => tokens.push(Token::SelfKw),
                        "result" => tokens.push(Token::ResultKw),
                        "error" => tokens.push(Token::ErrorKw),
                        "any" => tokens.push(Token::Any),
                        "i32" => tokens.push(Token::I32),
                        "i64" => tokens.push(Token::I64),
                        "f32" => tokens.push(Token::F32),
                        "f64" => tokens.push(Token::F64),
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
        
        // After processing the line, if we are not inside brackets and we added tokens on this line,
        // add a semicolon to terminate statements unless the line ends with something that explicitly
        // continues to the next line (like a colon).
        if nest_level == 0 && !tokens.is_empty() {
            let last = tokens.last().unwrap();
            match last {
                Token::Colon | Token::Comma | Token::Semicolon | Token::Indent | Token::Dedent | Token::ParenOpen | Token::BracketOpen | Token::BraceOpen | Token::Attribute(_) => {},
                _ => { tokens.push(Token::Semicolon); }
            }
        }
    }

    while indents.len() > 1 { indents.pop(); tokens.push(Token::Dedent); }
    tokens
}
