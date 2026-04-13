use chumsky::prelude::*;
use crate::ast::*;
use crate::lexer::Token;

type TokenStream<'a> = &'a [Token];

fn type_parser<'a>() -> impl Parser<'a, TokenStream<'a>, Type, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|ty| {
        let base = choice((
            just(Token::I32).to(Type::I32),
            just(Token::F32).to(Type::F32),
            just(Token::Bool).to(Type::Bool),
            just(Token::Str).to(Type::Str),
            just(Token::File).to(Type::File),
            just(Token::SelfKw).to(Type::SelfType),
            select! { Token::Ident(name) => Type::Custom(name, Vec::new()) },
        ));

        let ptr = just(Token::Star)
            .ignore_then(choice((
                just(Token::Box).to(AllocKind::Box),
                just(Token::Mut).to(AllocKind::RawMut),
                just(Token::Const).to(AllocKind::RawConst),
            )))
            .then(ty.clone())
            .map(|(kind, inner)| match kind {
                AllocKind::Box => Type::BoxPtr(Box::new(inner)),
                _ => Type::RawPtr(Box::new(inner), matches!(kind, AllocKind::RawMut)),
            });

        let array = ty.clone()
            .then_ignore(just(Token::Semicolon))
            .then(select! { Token::Int(v) => v as usize })
            .delimited_by(just(Token::BracketOpen), just(Token::BracketClose))
            .map(|(t, s)| Type::Array(Box::new(t), s));

        let custom_with_generics = select! { Token::Ident(name) => name }
            .then(ty.separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::Lt), just(Token::Gt)))
            .map(|(name, gens)| Type::Custom(name, gens));

        choice((custom_with_generics, ptr, array, base))
    })
}

fn expr_parser<'a>() -> impl Parser<'a, TokenStream<'a>, Expr, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|expr| {
        let val = select! {
            Token::Int(v) => Expr::Int(v),
            Token::Float(v) => Expr::Float(v),
            Token::Boolean(v) => Expr::Bool(v),
            Token::String(v) => Expr::String(v),
            Token::Ident(name) => Expr::Variable(name),
            Token::SelfKw => Expr::Variable("self".to_string()),
        };

        let alloc = just(Token::Star)
            .ignore_then(choice((
                just(Token::Box).to(AllocKind::Box),
                just(Token::Mut).to(AllocKind::RawMut),
                just(Token::Const).to(AllocKind::RawConst),
            )))
            .then(expr.clone())
            .map(|(kind, e)| Expr::Alloc(Box::new(e), kind));

        let array_init = expr.clone()
            .then_ignore(just(Token::Semicolon))
            .then(expr.clone())
            .delimited_by(just(Token::BracketOpen), just(Token::BracketClose))
            .map(|(v, s)| Expr::Call("array_init".to_string(), vec![v, s]));

        let struct_literal = select! { Token::Ident(name) => name }
            .or(just(Token::SelfKw).to("self".to_string()))
            .then(
                select! { Token::Ident(name) => name }
                    .then(just(Token::Colon).ignore_then(expr.clone()).or_not())
                    .map(|(name, val)| (name.clone(), val.unwrap_or(Expr::Variable(name))))
                    .separated_by(just(Token::Comma))
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::BraceOpen), just(Token::BraceClose))
            )
            .map(|(name, fields)| Expr::StructLiteral { name, fields });

        let term = choice((alloc, array_init, struct_literal, val, expr.clone().delimited_by(just(Token::ParenOpen), just(Token::ParenClose))));

        let suffix = choice((
            just(Token::Dot).ignore_then(select! { Token::Ident(name) => name })
                .then(expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)).or_not())
                .map(|(name, args)| (name, args, None)),
            expr.clone().delimited_by(just(Token::BracketOpen), just(Token::BracketClose))
                .map(|e| (String::new(), None, Some(e))),
        ));

        let suffixed = term.clone().foldl(suffix.repeated(), |lhs, (name, args, index): (String, Option<Vec<Expr>>, Option<Expr>)| {
            if let Some(idx) = index { Expr::IndexAccess(Box::new(lhs), Box::new(idx)) }
            else if let Some(arguments) = args { Expr::MethodCall(Box::new(lhs), name, arguments) }
            else { Expr::MemberAccess(Box::new(lhs), name) }
        });

        let call = select! { Token::Ident(name) => name }
            .then(just(Token::DoubleColon).ignore_then(select! { Token::Ident(name) => name }).or_not())
            .then(expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)))
            .map(|((name, method), args)| {
                if let Some(m) = method { Expr::Call(format!("{}::{}", name, m), args) }
                else { Expr::Call(name, args) }
            });

        let atom = choice((call, suffixed));

        let op = |t, op| just(t).to(op);
        let mul_op = op(Token::Star, BinaryOp::Multiply).or(op(Token::Div, BinaryOp::Divide));
        let add_op = op(Token::Plus, BinaryOp::Add).or(op(Token::Minus, BinaryOp::Subtract));
        let cmp_op = op(Token::Gt, BinaryOp::GreaterThan).or(op(Token::Lt, BinaryOp::LessThan));

        let product = atom.clone().foldl(mul_op.then(atom).repeated(), |lhs, (op, rhs)| {
            Expr::Binary(Box::new(lhs), op, Box::new(rhs))
        });

        let sum = product.clone().foldl(add_op.then(product).repeated(), |lhs, (op, rhs)| {
            Expr::Binary(Box::new(lhs), op, Box::new(rhs))
        });

        sum.clone().foldl(cmp_op.then(sum).repeated(), |lhs, (op, rhs)| {
            Expr::Binary(Box::new(lhs), op, Box::new(rhs))
        })
    })
}

pub fn parser<'a>() -> impl Parser<'a, TokenStream<'a>, Program, extra::Err<Rich<'a, Token>>> {
    let expr = expr_parser();
    let ty = type_parser();

    let stmt = recursive(|stmt| {
        let var_decl = just(Token::Var)
            .ignore_then(just(Token::Mut).or_not())
            .then(select! { Token::Ident(name) => name })
            .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
            .then_ignore(just(Token::Eq))
            .then(expr.clone())
            .then_ignore(just(Token::Semicolon).or_not())
            .map(|(((mut_kw, name), ty), value)| Stmt::VarDecl {
                name, is_mutable: mut_kw.is_some(), ty, value
            });

        let ptr_decl = just(Token::Star)
            .ignore_then(choice((
                just(Token::Mut).to(AllocKind::RawMut),
                just(Token::Const).to(AllocKind::RawConst),
                just(Token::Box).to(AllocKind::Box),
            )))
            .then(select! { Token::Ident(name) => name })
            .then_ignore(just(Token::Eq).or(just(Token::Colon)))
            .then(expr.clone())
            .then_ignore(just(Token::Semicolon).or_not())
            .map(|((kind, name), value)| Stmt::VarDecl {
                name, 
                is_mutable: match kind { AllocKind::RawMut | AllocKind::Box => true, _ => false },
                ty: None, 
                value: Expr::Alloc(Box::new(value), kind)
            });

        let var_decl_rev = expr.clone()
            .then_ignore(just(Token::Eq))
            .then_ignore(just(Token::Var))
            .then(just(Token::Mut).or_not())
            .then(select! { Token::Ident(name) => name })
            .then_ignore(just(Token::Semicolon).or_not())
            .map(|((value, mut_kw), name)| Stmt::VarDecl {
                name, is_mutable: mut_kw.is_some(), ty: None, value
            });

        let assign = expr.clone()
            .then_ignore(just(Token::Eq))
            .then(expr.clone())
            .then_ignore(just(Token::Semicolon).or_not())
            .map(|(target, value)| Stmt::Assign { target, value });

        let print_stmt = just(Token::Print)
            .ignore_then(expr.clone().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)))
            .then_ignore(just(Token::Semicolon).or_not())
            .map(Stmt::Print);

        let block = just(Token::Indent)
            .ignore_then(stmt.clone().repeated().collect())
            .then_ignore(just(Token::Dedent))
            .map(Stmt::Block);

        let if_stmt = just(Token::If).ignore_then(expr.clone()).then_ignore(just(Token::Colon)).then(block.clone())
            .then(just(Token::Else).ignore_then(just(Token::Colon).or_not()).ignore_then(block.clone()).or_not())
            .map(|((condition, then_branch), else_branch)| Stmt::If {
                condition, then_branch: Box::new(then_branch), else_branch: else_branch.map(Box::new),
            });

        let while_stmt = just(Token::While).ignore_then(expr.clone()).then_ignore(just(Token::Colon)).then(block.clone())
            .map(|(condition, body)| Stmt::While { condition, body: Box::new(body) });

        let expr_stmt = expr.clone().map(Stmt::ExprStmt).then_ignore(just(Token::Semicolon).or_not());

        choice((ptr_decl, var_decl, var_decl_rev, assign, print_stmt, block, if_stmt, while_stmt, expr_stmt))
    });

    let generic_params = select! { Token::Ident(name) => name }.separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::Lt), just(Token::Gt));

    let param = just(Token::Mut).or_not()
        .then(choice((select! { Token::Ident(name) => name }, just(Token::SelfKw).to("self".to_string()))))
        .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
        .map(|((mut_kw, name), ty): ((Option<Token>, String), Option<Type>)| Param { 
            name, 
            ty: ty.unwrap_or(Type::SelfType), 
            is_mutable: mut_kw.is_some() 
        });

    let func_parser = just(Token::Fn).ignore_then(select! { Token::Ident(name) => name })
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(param.separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)))
        .then(just(Token::Arrow).ignore_then(ty.clone()).or_not())
        .then_ignore(just(Token::Colon).or_not())
        .then(just(Token::Indent).ignore_then(stmt.clone().repeated().collect()).then_ignore(just(Token::Dedent)).map(Stmt::Block))
        .map(|((((name, generics), params), return_type), body)| Function { name, generics, params, return_type, body });

    let obj_parser = just(Token::Obj).ignore_then(select! { Token::Ident(name) => name })
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then_ignore(just(Token::Colon).or_not())
        .then(
            just(Token::Indent)
                .ignore_then(select! { Token::Ident(name) => name }.then_ignore(just(Token::Colon)).then(ty.clone()).map(|(name, ty)| Field { name, ty }).repeated().collect())
                .then_ignore(just(Token::Dedent))
        ).map(|((name, generics), fields)| ObjectDecl { name, generics, fields });

    let impl_parser = just(Token::Impl)
        .ignore_then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(select! { Token::Ident(name) => name })
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then_ignore(just(Token::Colon).or_not())
        .then(just(Token::Indent).ignore_then(func_parser.clone().repeated().collect()).then_ignore(just(Token::Dedent)))
        .map(|(((_impl_gens, target), _target_gens), functions)| ImplDecl { target, generics: _impl_gens, functions });

    let decl = choice((func_parser.map(Decl::Function), obj_parser.map(Decl::Object), impl_parser.map(Decl::Impl)));

    decl.repeated().collect().map(|declarations| Program { declarations }).then_ignore(end())
}
