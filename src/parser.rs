use chumsky::prelude::*;
use crate::ast::*;

// ==========================================
// 2. The Parser (Updated for 0.12.0)
// ==========================================
pub fn parser<'a>() -> impl Parser<'a, &'a str, Function, extra::Err<Rich<'a, char>>> {
    // 1. Parse integers: "42"
    let int = text::int(10)
        .map(|s: &str| Expr::Int(s.parse().unwrap()))
        .padded();

    // 2. Parse a print statement: "print(42);"
    let print_stmt = text::keyword("print")
        .padded()
        .ignore_then(int.delimited_by(just('('), just(')')))
        .then_ignore(just(';'))
        .map(Stmt::Print)
        .padded();

    // 3. Parse a function: "fn main() { ... }"
    let function = text::keyword("fn")
        .padded()
        .ignore_then(text::ident())
        .then_ignore(just("()").padded())
        .then(
            print_stmt
                .repeated()
                .collect::<Vec<_>>() // Need to collect in 0.12+
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map(|(name, body): (&str, Vec<Stmt>)| Function {
            name: name.to_string(),
            body,
        })
        .padded();

    function.then_ignore(end())
}
