use chumsky::prelude::*;
use crate::ast::*;

// ==========================================
// 2. The Parser (Updated for 0.12.0)
// ==========================================
pub fn parser<'a>() -> impl Parser<'a, &'a str, Function, extra::Err<Rich<'a, char>>> {
    // 1. Types: "i32"
    let ty = text::keyword("i32").to(Type::I32).padded();

    // 2. Variable names: "x"
    let var_name = text::ident().map(|s: &str| s.to_string()).padded();

    // 3. Parse integers: "42"
    let int = text::int(10)
        .map(|s: &str| Expr::Int(s.parse().unwrap()))
        .padded();

    // 4. Expressions: "42" or "x"
    let expr = int.or(var_name.clone().map(Expr::Variable));

    // 5. Variable declarations: "var x: i32 = 42;" or "var mut x: i32 = 42;"
    let var_decl = text::keyword("var")
        .padded()
        .ignore_then(text::keyword("mut").padded().or_not())
        .then(var_name)
        .then_ignore(just(':').padded())
        .then(ty)
        .then_ignore(just('=').padded())
        .then(expr)
        .then_ignore(just(';').padded())
        .map(|(((mut_kw, name), ty), value)| Stmt::VarDecl {
            name,
            is_mutable: mut_kw.is_some(),
            ty,
            value,
        });

    // 6. Print statement: "print(x);"
    let print_stmt = text::keyword("print")
        .padded()
        .ignore_then(expr.delimited_by(just('('), just(')')))
        .then_ignore(just(';').padded())
        .map(Stmt::Print)
        .padded();

    let stmt = var_decl.or(print_stmt);

    // 7. Functions: "fn main() { ... }"
    let function = text::keyword("fn")
        .padded()
        .ignore_then(text::ident())
        .then_ignore(just("()").padded())
        .then(
            stmt.repeated()
                .collect::<Vec<_>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map(|(name, body): (&str, Vec<Stmt>)| Function {
            name: name.to_string(),
            body,
        })
        .padded();

    function.then_ignore(end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_decl() {
        let input = "fn main() { var x: i32 = 42; print(x); }";
        let result = parser().parse(input).into_result();
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    #[test]
    fn test_var_mut_decl() {
        let input = "fn main() { var mut x: i32 = 42; print(x); }";
        let result = parser().parse(input).into_result();
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }
}
