use proc_macro2::TokenStream;
use quote::quote;

/// Finds the contents of matching brackets, respecting nested depth.
/// Example: "HashMap<String, Vec<i32>>::iter" -> Some(("HashMap", "String, Vec<i32>", "::iter"))
pub fn extract_brackets<'a>(input: &'a str, open: char, close: char) -> Option<(&'a str, &'a str, &'a str)> {
    let start_idx = input.find(open)?;
    let mut depth = 0;

    for (i, c) in input[start_idx..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                let absolute_idx = start_idx + i;
                return Some((
                    &input[..start_idx],                 // Before the bracket
                    &input[start_idx + 1..absolute_idx], // Inside the brackets
                    &input[absolute_idx + 1..],          // After the bracket
                ));
            }
        }
    }
    None
}

/// Splits a comma-separated string, ignoring commas inside nested `< >` or `( )`.
pub fn parse_generic_args(args_str: &str) -> Vec<TokenStream> {
    let mut tokens = Vec::new();
    let mut current_arg = String::new();
    let mut depth = 0;

    for c in args_str.chars() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                tokens.push(parse_single_generic(&current_arg));
                current_arg.clear();
                continue;
            }
            _ => {}
        }
        current_arg.push(c);
    }

    if !current_arg.trim().is_empty() {
        tokens.push(parse_single_generic(&current_arg));
    }

    tokens
}

/// Parses a single generic parameter
pub fn parse_single_generic(s: &str) -> TokenStream {
    let s = s.trim();
    if s.is_empty() {
        return quote!();
    }
    
    // If it's a simple generic parameter like "T", format it as an ident
    if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        let gid = quote::format_ident!("{}", s);
        quote!(#gid)
    } else {
        // Otherwise try to parse it (it might be a complex type)
        s.parse::<TokenStream>().unwrap_or_else(|_| {
            let gid = quote::format_ident!("{}", s);
            quote!(#gid)
        })
    }
}

/// Helper to sanitize a slice of namespace parts, dropping empty strings and trailing `<>`
pub fn format_path_parts(parts: &[&str]) -> Vec<TokenStream> {
    parts.iter()
        .filter(|s| !s.is_empty())
        .map(|s| {
            // Replaces your old allocation-heavy `replace("<>", "")`
            let clean = s.trim_end_matches("<>");
            let id = quote::format_ident!("{}", clean);
            quote!(#id)
        })
        .collect()
}
