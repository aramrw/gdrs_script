use crate::ast::*;
use crate::lexer::{Span, Token};
use crate::parser::types::type_parser;
use crate::parser::{ExprParserExt, ParserExt, ParserExtra, ident};
use chumsky::input::ValueInput;
use chumsky::prelude::*;

pub fn expr_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
    expr: impl Parser<'a, I, Expr, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, Expr, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    let ty = type_parser::<I>();

    // -------------------------------------------------------------------------
    // 0. High-Precedence Primitives & Path Helpers
    // -------------------------------------------------------------------------
    let block_expr = stmt
        .clone()
        .repeated()
        .collect::<Vec<_>>()
        .braces()
        .map(ExprKind::Block)
        .into_expr();

    let type_args = ty
        .clone()
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::Lt), just(Token::Gt))
        .or_not()
        .map(|g| g.unwrap_or_default());

    let path_part = ident()
        .then(type_args)
        .map(|(name, generics)| PathPart { name, generics });

    let identifier_path = path_part
        .clone()
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>();

    let val = choice((
        just(Token::ParenOpen)
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(Token::ParenClose))
            .map(|(_, items): (Token, Vec<Expr>)| {
                if items.is_empty() {
                    ExprKind::Unit
                } else if items.len() == 1 {
                    let first = items.into_iter().next().unwrap();
                    first.kind
                } else {
                    ExprKind::Tuple(items)
                }
            }),
        select! { Token::Int(v) => ExprKind::Int(v) },
        select! { Token::Int64(v) => ExprKind::Int64(v) },
        select! { Token::Float(v) => ExprKind::Float(v) },
        select! { Token::Float64(v) => ExprKind::Float64(v) },
        select! { Token::Boolean(v) => ExprKind::Bool(v) },
        select! { Token::String(v) => ExprKind::String(v) },
        identifier_path.clone().map(ExprKind::Variable),
        just(Token::SelfKw).to(ExprKind::Variable(vec![PathPart {
            name: "self".to_string(),
            generics: Vec::new(),
        }])),
    ))
    .into_expr();

    let array_init_or_literal = choice((
        expr.clone()
            .then_ignore(just(Token::Semicolon))
            .then(expr.clone())
            .map(|(v, s)| {
                ExprKind::Call(
                    vec![PathPart {
                        name: "array_init".to_string(),
                        generics: Vec::new(),
                    }],
                    vec![v, s],
                    None,
                    None,
                    None,
                )
            }),
        expr.clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .map(ExprKind::Array),
    ))
    .brackets()
    .into_expr();

    let struct_literal = identifier_path
        .clone()
        .or(just(Token::SelfKw).to(vec![PathPart {
            name: "self".to_string(),
            generics: Vec::new(),
        }]))
        .then(
            ident()
                .then(just(Token::Colon).ignore_then(expr.clone()).or_not())
                .map_with(|(name, val), e| {
                    (
                        name.clone(),
                        val.unwrap_or(Expr {
                            kind: ExprKind::Variable(vec![PathPart {
                                name,
                                generics: Vec::new(),
                            }]),
                            span: e.span(),
                            ty: None,
                        }),
                    )
                })
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .braces(),
        )
        .map(|(path, fields)| ExprKind::StructLiteral {
            path,
            fields,
            resolved_name: None,
        })
        .into_expr();

    let call_path = identifier_path
        .clone()
        .or(just(Token::Str).to(vec![PathPart {
            name: "str".into(),
            generics: vec![],
        }]));

    let namespaced_call = call_path
        .clone()
        .then(choice((
            just(Token::Bang)
                .ignore_then(choice((
                    expr.clone()
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .parens(),
                    expr.clone()
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .brackets(),
                )))
                .map(|args| (true, args)),
            expr.clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .parens()
                .map(|args| (false, args)),
        )))
        .map(|(path, (is_macro, args))| {
            if is_macro {
                let full_name = path
                    .into_iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>()
                    .join("::");
                ExprKind::MacroCall(full_name, args)
            } else {
                ExprKind::Call(path, args, None, None, None)
            }
        })
        .into_expr();

    // -------------------------------------------------------------------------
    // 1. Base Terms
    // -------------------------------------------------------------------------
    let base_term = choice((
        struct_literal,
        namespaced_call,
        array_init_or_literal,
        block_expr,
        val,
    ));

    // -------------------------------------------------------------------------
    // 2. Suffixes (Member Access `.a`, Method Calls `.f()`, Indexing `[i]`, etc.)
    // -------------------------------------------------------------------------
    #[derive(Clone)]
    enum Suffix {
        Method(String, Option<Vec<Expr>>),
        Index(Expr),
        Try,
        TildeUnwrap,
        Await,
    }

    let suffix = choice((
        just(Token::Dot)
            .ignore_then(choice((
                ident(),
                select! { Token::Int(v) => v.to_string() },
            )))
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .collect::<Vec<_>>()
                    .parens()
                    .or_not(),
            )
            .map(|(name, args)| Suffix::Method(name, args)),
        expr.clone().brackets().map(Suffix::Index),
        just(Token::QuestionMark).to(Suffix::Try),
        just(Token::Tilde).to(Suffix::TildeUnwrap),
        just(Token::Dot)
            .ignore_then(just(Token::Await))
            .to(Suffix::Await),
    ));

    // -------------------------------------------------------------------------
    // 3. Atom (Base term with attached suffixes)
    // -------------------------------------------------------------------------
    let atom = base_term.clone().foldl(suffix.repeated(), |lhs, suff| {
        let span = lhs.span;
        match suff {
            Suffix::Await => Expr {
                kind: ExprKind::Await(Box::new(lhs)),
                span,
                ty: None,
            },
            Suffix::TildeUnwrap => Expr {
                kind: ExprKind::Unwrap(Box::new(lhs)),
                span,
                ty: None,
            },
            Suffix::Try => Expr {
                kind: ExprKind::Try(Box::new(lhs)),
                span,
                ty: None,
            },
            Suffix::Index(idx) => Expr {
                kind: ExprKind::IndexAccess(Box::new(lhs), Box::new(idx)),
                span,
                ty: None,
            },
            Suffix::Method(name, Some(args)) => Expr {
                kind: ExprKind::MethodCall(Box::new(lhs), name, args, None, None, None),
                span,
                ty: None,
            },
            Suffix::Method(name, None) => Expr {
                kind: ExprKind::MemberAccess(Box::new(lhs), name),
                span,
                ty: None,
            },
        }
    });

    // -------------------------------------------------------------------------
    // 4. Prefixes (&, *, -, ~) — Applied specifically to `atom`
    // -------------------------------------------------------------------------
    let alloc_or_deref = just(Token::Star)
        .ignore_then(choice((
            just(Token::Box)
                .to(AllocKind::Box)
                .then(expr.clone().parens())
                .map(|(_, e)| ExprKind::Alloc(Box::new(e), AllocKind::Box)),
            just(Token::Mut)
                .to(AllocKind::RawMut)
                .then(atom.clone())
                .map(|(_, e)| ExprKind::Alloc(Box::new(e), AllocKind::RawMut)),
            just(Token::Const)
                .to(AllocKind::RawConst)
                .then(atom.clone())
                .map(|(_, e)| ExprKind::Alloc(Box::new(e), AllocKind::RawConst)),
            just(Token::Rc)
                .to(AllocKind::Rc)
                .then(expr.clone().parens())
                .map(|(_, e)| ExprKind::Alloc(Box::new(e), AllocKind::Rc)),
            just(Token::Arc)
                .to(AllocKind::Arc)
                .then(expr.clone().parens())
                .map(|(_, e)| ExprKind::Alloc(Box::new(e), AllocKind::Arc)),
            atom.clone().map(|e| ExprKind::Deref(Box::new(e))),
        )))
        .into_expr()
        .boxed();

    let borrow = just(Token::Amp)
        .ignore_then(just(Token::Mut).or_not())
        .then(atom.clone())
        .map(|(mut_kw, e)| ExprKind::Borrow(Box::new(e), mut_kw.is_some()))
        .into_expr();

    let negate = just(Token::Minus)
        .ignore_then(atom.clone())
        .map(|e| ExprKind::Negate(Box::new(e)))
        .into_expr();

    let downgrade = just(Token::Tilde)
        .ignore_then(atom.clone())
        .map(|e| ExprKind::Downgrade(Box::new(e)))
        .into_expr();

    let prefix_expr = choice((alloc_or_deref, borrow, negate, downgrade, atom));

    // -------------------------------------------------------------------------
    // 5. Casts (`as T`) — Binds broader than prefixes, narrower than binary ops
    // -------------------------------------------------------------------------
    let cast_expr = prefix_expr.clone().foldl(
        just(Token::As).ignore_then(ty.clone()).repeated(),
        |lhs, target_ty| {
            let span = lhs.span;
            Expr {
                kind: ExprKind::Cast(Box::new(lhs), target_ty),
                span,
                ty: None,
            }
        },
    );

    // -------------------------------------------------------------------------
    // 6. Binary Operators
    // -------------------------------------------------------------------------
    let mul_op = choice((
        just(Token::Star).to(BinaryOp::Multiply),
        just(Token::MulAssign).to(BinaryOp::MulAssign),
        just(Token::Div).to(BinaryOp::Divide),
        just(Token::DivAssign).to(BinaryOp::DivAssign),
        just(Token::Modulo).to(BinaryOp::Modulo),
    ));

    let add_op = just(Token::Plus)
        .to(BinaryOp::Add)
        .or(just(Token::Minus).to(BinaryOp::Subtract))
        .or(just(Token::AddAssign).to(BinaryOp::AddAssign));

    let cmp_op = choice((
        just(Token::GtEq).to(BinaryOp::GreaterThanOrEqual),
        just(Token::LtEq).to(BinaryOp::LessThanOrEqual),
        just(Token::DoubleEq).to(BinaryOp::Equal),
        just(Token::Gt).to(BinaryOp::GreaterThan),
        just(Token::Lt).to(BinaryOp::LessThan),
    ));

    let product = cast_expr
        .clone()
        .foldl(mul_op.then(cast_expr).repeated(), |lhs, (op, rhs)| {
            let span = lhs.span;
            Expr {
                kind: ExprKind::Binary(Box::new(lhs), op, Box::new(rhs)),
                span,
                ty: None,
            }
        });

    let sum = product
        .clone()
        .foldl(add_op.then(product).repeated(), |lhs, (op, rhs)| {
            let span = lhs.span;
            Expr {
                kind: ExprKind::Binary(Box::new(lhs), op, Box::new(rhs)),
                span,
                ty: None,
            }
        });

    let comparison = sum
        .clone()
        .foldl(cmp_op.then(sum).repeated(), |lhs, (op, rhs)| {
            let span = lhs.span;
            Expr {
                kind: ExprKind::Binary(Box::new(lhs), op, Box::new(rhs)),
                span,
                ty: None,
            }
        });

    comparison.clone().foldl(
        just(Token::Or).to(BinaryOp::Or).then(comparison).repeated(),
        |lhs, (op, rhs)| {
            let span = lhs.span;
            Expr {
                kind: ExprKind::Binary(Box::new(lhs), op, Box::new(rhs)),
                span,
                ty: None,
            }
        },
    )
}
