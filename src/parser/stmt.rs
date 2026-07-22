use crate::ast::*;
use crate::lexer::{Span, Token};
use crate::parser::expr::expr_parser;
use crate::parser::types::type_parser;
use crate::parser::{ParserExt, ParserExtra, StmtParserExt, double_colon_path, ident};
use chumsky::input::ValueInput;
use chumsky::prelude::*;

pub fn stmt_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
    expr: impl Parser<'a, I, Expr, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    let ty = type_parser::<I>();

    let block = just(Token::Indent)
        .ignore_then(stmt.clone().repeated().collect())
        .then_ignore(just(Token::Dedent))
        .map(StmtKind::Block)
        .into_stmt();

    let var_decl = just(Token::Var)
        .ignore_then(just(Token::Mut).or_not())
        .then(ident())
        .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
        .then_ignore(just(Token::Eq))
        .then(expr.clone())
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|(((mut_kw, name), ty), value)| StmtKind::VarDecl {
            name,
            is_mutable: mut_kw.is_some(),
            ty,
            value,
        })
        .into_stmt();

    let ptr_decl = just(Token::Star)
        .ignore_then(choice((
            just(Token::Mut).to(AllocKind::RawMut),
            just(Token::Const).to(AllocKind::RawConst),
            just(Token::Box).to(AllocKind::Box),
            just(Token::Rc).to(AllocKind::Rc),
            just(Token::Arc).to(AllocKind::Arc),
        )))
        .then(ident())
        .then_ignore(just(Token::Eq).or(just(Token::Colon)))
        .then(expr.clone())
        .then_ignore(just(Token::Semicolon).or_not())
        .map_with(|((kind, name), value), e| {
            let is_mutable = matches!(
                kind,
                AllocKind::RawMut | AllocKind::Box | AllocKind::Rc | AllocKind::Arc
            );
            let alloc_expr = Expr {
                kind: ExprKind::Alloc(Box::new(value), kind),
                span: e.span(),
                ty: None,
            };
            StmtKind::VarDecl {
                name,
                is_mutable,
                ty: None,
                value: alloc_expr,
            }
        })
        .into_stmt();

    let assign_op = choice((
        just(Token::Eq),
        just(Token::AddAssign),
        just(Token::SubAssign),
        just(Token::MulAssign),
        just(Token::DivAssign),
    ));

    let assign = expr
        .clone()
        .then(assign_op)
        .then(expr.clone())
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|((target, _op), value)| StmtKind::Assign { target, value }) // or build your compound AST node here
        .into_stmt();

    let if_stmt = just(Token::If)
        .ignore_then(expr.clone())
        .then_ignore(just(Token::Colon))
        .then(block.clone())
        .then(
            just(Token::Else)
                .ignore_then(just(Token::Colon).or_not())
                .ignore_then(block.clone())
                .or_not(),
        )
        .map(|((condition, then_branch), else_branch)| StmtKind::If {
            condition,
            then_branch: Box::new(then_branch),
            else_branch: else_branch.map(Box::new),
        })
        .into_stmt();

    let while_stmt = just(Token::While)
        .ignore_then(expr.clone())
        .then_ignore(just(Token::Colon))
        .then(block.clone())
        .map(|(condition, body)| StmtKind::While {
            condition,
            body: Box::new(body),
        })
        .into_stmt();

    let for_stmt = just(Token::For)
        .ignore_then(ident())
        .then_ignore(just(Token::In))
        .then(expr.clone())
        .then_ignore(just(Token::Colon))
        .then(block.clone())
        .map(|((var_name, iterator), body)| StmtKind::For {
            var_name,
            iterator,
            body: Box::new(body),
        })
        .into_stmt();

    let loop_stmt = just(Token::Loop)
        .ignore_then(just(Token::Colon).or_not())
        .then(block.clone())
        .map(|(_, body)| StmtKind::Loop {
            body: Box::new(body),
        })
        .into_stmt();

    let break_stmt = just(Token::Break)
        .ignore_then(expr.clone().or_not())
        .then_ignore(just(Token::Semicolon).or_not())
        .map(StmtKind::Break)
        .into_stmt();

    let return_stmt = just(Token::Return)
        .ignore_then(expr.clone().or_not())
        .then_ignore(just(Token::Semicolon).or_not())
        .map(StmtKind::Return)
        .into_stmt();

    let match_pattern = choice((
        double_colon_path()
            .then(
                ident()
                    .separated_by(just(Token::Comma))
                    .collect::<Vec<_>>()
                    .parens()
                    .or_not(),
            )
            .map(|(path, params)| {
                let mut parts: Vec<&str> = path.split("::").collect();
                if let Some(p) = params {
                    if parts.len() == 1 {
                        Pattern::Variant(String::new(), parts[0].to_string(), p)
                    } else {
                        let var = parts.pop().unwrap().to_string();
                        Pattern::Variant(parts.join("::"), var, p)
                    }
                } else {
                    if parts.len() == 1 {
                        Pattern::Variable(parts[0].to_string())
                    } else {
                        let var = parts.pop().unwrap().to_string();
                        Pattern::Variant(parts.join("::"), var, Vec::new())
                    }
                }
            }),
        expr.clone().map(Pattern::Literal),
    ));

    let match_stmt = just(Token::Match)
        .ignore_then(expr.clone())
        .then_ignore(choice((
            just(Token::FatArrow),
            just(Token::Colon),
            empty().to(Token::Colon), // Fallback
        )))
        .then(
            just(Token::Indent)
                .ignore_then(
                    match_pattern
                        .then_ignore(just(Token::Colon))
                        .then(choice((block.clone(), stmt.clone())))
                        .map(|(pattern, body)| Arm { pattern, body })
                        .repeated()
                        .collect(),
                )
                .then_ignore(just(Token::Dedent)),
        )
        .map(|(expr, arms)| StmtKind::Match { expr, arms })
        .into_stmt();

    let unsafe_stmt = just(Token::Ident("unsafe".to_string()))
        .ignore_then(just(Token::Colon).or_not())
        .ignore_then(just(Token::Indent))
        .ignore_then(stmt.clone().repeated().collect())
        .then_ignore(just(Token::Dedent))
        .map(StmtKind::UnsafeBlock)
        .into_stmt();

    let expr_stmt = expr
        .clone()
        .map(StmtKind::ExprStmt)
        .into_stmt()
        .then_ignore(just(Token::Semicolon).or_not());

    choice((
        ptr_decl,
        var_decl,
        assign,
        block,
        if_stmt,
        while_stmt,
        for_stmt,
        loop_stmt,
        break_stmt,
        return_stmt,
        match_stmt,
        unsafe_stmt,
        expr_stmt,
    ))
}
