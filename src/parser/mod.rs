use chumsky::prelude::*;
use crate::ast::*;
use crate::lexer::Token;

type TokenStream<'a> = &'a [Token];

fn type_parser<'a>() -> impl Parser<'a, TokenStream<'a>, Type, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|ty| {
        let base = choice((
            just(Token::I32).to(Type::I32),
            just(Token::I64).to(Type::I64),
            just(Token::F32).to(Type::F32),
            just(Token::F64).to(Type::F64),
            just(Token::Bool).to(Type::Bool),
            just(Token::Str).to(Type::Str),
            just(Token::StringKw).to(Type::String),
            just(Token::File).to(Type::File),
            just(Token::SelfKw).to(Type::SelfType),
            just(Token::ResultKw).ignore_then(
                ty.clone()
                    .then_ignore(just(Token::Comma))
                    .then(ty.clone())
                    .delimited_by(just(Token::Lt), just(Token::Gt))
            ).map(|(ok, err)| Type::Result(Box::new(ok), Box::new(err))),
            just(Token::ErrorKw).to(Type::Error),
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
            .then(just(Token::DoubleColon).ignore_then(select! { Token::Ident(name) => name }).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                let mut full = first;
                for part in rest {
                    full.push_str("::");
                    full.push_str(&part);
                }
                full
            })
            .then(ty.separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::Lt), just(Token::Gt)).or_not())
            .map(|(name, gens)| Type::Custom(name, gens.unwrap_or_default()));

        choice((custom_with_generics, ptr, array, base))
    })
}

fn expr_parser<'a>() -> impl Parser<'a, TokenStream<'a>, Expr, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|expr| {
        let val = select! {
            Token::Int(v) => Expr::Int(v),
            Token::Int64(v) => Expr::Int64(v),
            Token::Float(v) => Expr::Float(v),
            Token::Float64(v) => Expr::Float64(v),
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

        let call = select! { Token::Ident(name) => name }
            .then(just(Token::DoubleColon).ignore_then(select! { Token::Ident(name) => name }).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                let mut full = first;
                for part in rest {
                    full.push_str("::");
                    full.push_str(&part);
                }
                full
            })
            .then(expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)))
            .map(|(name, args)| Expr::Call(name, args));

        let term = choice((call, alloc, array_init, struct_literal, val, expr.clone().delimited_by(just(Token::ParenOpen), just(Token::ParenClose))));

        let suffix = choice((
            just(Token::Dot).ignore_then(select! { Token::Ident(name) => name })
                .then(expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)).or_not())
                .map(|(name, args)| (name, args, None, false, false)),
            expr.clone().delimited_by(just(Token::BracketOpen), just(Token::BracketClose))
                .map(|e| (String::new(), None, Some(e), false, false)),
            just(Token::QuestionMark).to((String::new(), None, None, true, false)),
            just(Token::Dot).ignore_then(just(Token::Await)).to((String::new(), None, None, false, true)),
        ));

        let atom = term.clone().foldl(suffix.repeated(), |lhs, (name, args, index, unwrap, is_await): (String, Option<Vec<Expr>>, Option<Expr>, bool, bool)| {
            if is_await { Expr::Await(Box::new(lhs)) }
            else if unwrap { Expr::Unwrap(Box::new(lhs)) }
            else if let Some(idx) = index { Expr::IndexAccess(Box::new(lhs), Box::new(idx)) }
            else if let Some(arguments) = args { Expr::MethodCall(Box::new(lhs), name, arguments) }
            else { Expr::MemberAccess(Box::new(lhs), name) }
        });

        let op = |t, op| just(t).to(op);
        let mul_op = op(Token::Star, BinaryOp::Multiply).or(op(Token::Div, BinaryOp::Divide));
        let add_op = op(Token::Plus, BinaryOp::Add).or(op(Token::Minus, BinaryOp::Subtract));
        let cmp_op = op(Token::Gt, BinaryOp::GreaterThan).or(op(Token::Lt, BinaryOp::LessThan)).or(op(Token::DoubleEq, BinaryOp::Equal));

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

        let return_stmt = just(Token::Return).ignore_then(expr.clone().or_not())
            .then_ignore(just(Token::Semicolon).or_not())
            .map(Stmt::Return);

        let match_stmt = just(Token::Match).ignore_then(expr.clone())
            .then_ignore(just(Token::Colon).or_not())
            .then(
                just(Token::Indent)
                    .ignore_then(
                        select! { Token::Ident(name) => name }
                            .then(just(Token::DoubleColon).ignore_then(select! { Token::Ident(name) => name }).repeated().collect::<Vec<_>>())
                            .map(|(first, mut rest)| {
                                if rest.is_empty() {
                                    (first, None)
                                } else {
                                    let var = rest.pop().unwrap();
                                    let mut enm = first;
                                    for part in rest {
                                        enm.push_str("::");
                                        enm.push_str(&part);
                                    }
                                    (enm, Some(var))
                                }
                            })
                            .then(select! { Token::Ident(name) => name }.separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)).or_not())
                            .map(|((enm, var), params)| {
                                if let Some(v) = var { Pattern::Variant(enm, v, params.unwrap_or_default()) }
                                else { Pattern::Variable(enm) }
                            })
                            .or(expr.clone().map(Pattern::Literal))
                            .then_ignore(just(Token::FatArrow))
                            .then(stmt.clone())
                            .map(|(pattern, body)| Arm { pattern, body })
                            .repeated().collect()
                    )
                    .then_ignore(just(Token::Dedent))
            )
            .map(|(expr, arms)| Stmt::Match { expr, arms });

        let expr_stmt = expr.clone().map(Stmt::ExprStmt).then_ignore(just(Token::Semicolon).or_not());

        choice((ptr_decl, var_decl, assign, print_stmt, block, if_stmt, while_stmt, return_stmt, match_stmt, expr_stmt))
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

    let rust_path = just(Token::Eq).ignore_then(select! { Token::String(s) => s });

    let attribute_parser = select! { Token::Attribute(s) => s }.repeated().collect::<Vec<_>>();

    let func_sig = just(Token::Async).or_not().map(|a| a.is_some())
        .then_ignore(just(Token::Fn))
        .then(select! { Token::Ident(name) => name })
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(param.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)))
        .then(just(Token::Arrow).ignore_then(ty.clone()).or_not())
        .then(rust_path.clone().or_not());

    let func_parser = attribute_parser.clone()
        .then(func_sig.clone())
        .then_ignore(just(Token::Colon).or_not())
        .then(just(Token::Indent).ignore_then(stmt.clone().repeated().collect()).then_ignore(just(Token::Dedent)).map(Stmt::Block))
        .map(|((attributes, (((((is_async, name), generics), params), return_type), rust_path)), body)| Function { name, generics, params, return_type, body, is_async, rust_path, attributes });

    let extern_func_parser = attribute_parser.clone()
        .then(just(Token::Extern).ignore_then(func_sig.clone()))
        .map(|(attributes, (((((is_async, name), generics), params), return_type), rust_path))| Function { 
            name, generics, params, return_type, is_async, rust_path, attributes,
            body: Stmt::Block(Vec::new()) 
        });

    let obj_inner = just(Token::Obj).ignore_then(select! { Token::Ident(name) => name })
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(rust_path.clone().or_not())
        .then_ignore(just(Token::Colon).or_not())
        .then(
            just(Token::Indent)
                .ignore_then(select! { Token::Ident(name) => name }.then_ignore(just(Token::Colon)).then(ty.clone()).map(|(name, ty)| Field { name, ty }).repeated().collect())
                .then_ignore(just(Token::Dedent))
                .or_not()
        );

    let obj_parser = attribute_parser.clone()
        .then(obj_inner.clone())
        .map(|(attributes, (((name, generics), rust_path), fields))| ObjectDecl { name, generics, fields: fields.unwrap_or_default(), rust_path, attributes });

    let extern_obj_parser = just(Token::Extern).ignore_then(obj_parser.clone());

    let enum_inner = just(Token::Enum).ignore_then(select! { Token::Ident(name) => name })
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(rust_path.clone().or_not())
        .then_ignore(just(Token::Colon).or_not())
        .then(
            just(Token::Indent)
                .ignore_then(
                    select! { Token::Ident(name) => name }
                        .then(ty.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)).or_not())
                        .map(|(name, types)| Variant { name, types: types.unwrap_or_default() })
                        .repeated().collect()
                )
                .then_ignore(just(Token::Dedent))
        );

    let enum_parser = attribute_parser.clone()
        .then(enum_inner.clone())
        .map(|(attributes, (((name, generics), rust_path), variants))| EnumDecl { name, generics, variants, rust_path, attributes });

    let extern_enum_parser = just(Token::Extern).ignore_then(enum_parser.clone());

    let impl_parser = just(Token::Impl)
        .ignore_then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(
            select! { Token::Ident(name) => name }
                .then(just(Token::DoubleColon).ignore_then(select! { Token::Ident(name) => name }).repeated().collect::<Vec<_>>())
                .map(|(first, rest)| {
                    let mut full = first;
                    for part in rest {
                        full.push_str("::");
                        full.push_str(&part);
                    }
                    full
                })
        )
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then_ignore(just(Token::Colon).or_not())
        .then(just(Token::Indent).ignore_then(func_parser.clone().repeated().collect()).then_ignore(just(Token::Dedent)))
        .map(|(((_impl_gens, target), _target_gens), functions)| ImplDecl { target, generics: _impl_gens, functions });

    let extern_impl_parser = just(Token::Extern).ignore_then(just(Token::Impl))
        .ignore_then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(select! { Token::Ident(name) => name }) // Simpler target for now
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then_ignore(just(Token::Colon).or_not())
        .then(
            just(Token::Indent)
                .ignore_then(
                    just(Token::Fn).ignore_then(select! { Token::Ident(name) => name })
                        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
                        .then(param.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)))
                        .then(just(Token::Arrow).ignore_then(ty.clone()).or_not())
                        .map(|(((name, generics), params), return_type)| Function { 
                            name, generics, params, return_type, 
                            body: Stmt::Block(Vec::new()),
                            is_async: false, rust_path: None,
                            attributes: Vec::new()
                        })
                        .repeated().collect()
                )
                .then_ignore(just(Token::Dedent))
        )
        .map(|(((_impl_gens, target), _target_gens), functions)| ImplDecl { target, generics: _impl_gens, functions });

    let use_parser = just(Token::Use).ignore_then(
        select! { Token::Ident(name) => name }
            .separated_by(just(Token::DoubleColon))
            .at_least(1)
            .collect::<Vec<_>>()
    )
        .then_ignore(just(Token::Semicolon).or_not())
        .map(Decl::Use);

    let rust_dependency_parser = just(Token::Rust).ignore_then(just(Token::Dependency))
        .ignore_then(select! { Token::Ident(name) => name }.or(select! { Token::String(name) => name }))
        .then_ignore(just(Token::Eq))
        .then(choice((
            select! { Token::String(version) => version },
            just(Token::BraceOpen)
                .ignore_then(any().and_is(just(Token::BraceClose).not()).repeated())
                .then_ignore(just(Token::BraceClose))
                .map(|_tokens| {
                    "{ version = \"1.0\", features = [\"derive\"] }".to_string() 
                })
        )))
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|(name, version)| Decl::RustDependency(name, version));

    let rust_block_parser = just(Token::Rust).ignore_then(just(Token::Colon))
        .ignore_then(just(Token::Indent))
        .ignore_then(any().repeated().collect::<Vec<_>>()) // This is tricky, simplified for now
        .then_ignore(just(Token::Dedent))
        .map(|_tokens| Decl::RustBlock("// Raw rust code injection not fully implemented in parser yet".to_string()));

    let decl = choice((
        rust_dependency_parser,
        rust_block_parser,
        func_parser.map(Decl::Function), 
        obj_parser.map(Decl::Object), 
        enum_parser.map(Decl::Enum), 
        impl_parser.map(Decl::Impl), 
        extern_func_parser.map(Decl::ExternFunction), 
        extern_obj_parser.map(Decl::ExternObject), 
        extern_enum_parser.map(Decl::ExternEnum),
        extern_impl_parser.map(Decl::ExternImpl),
        use_parser
    ));

    decl.repeated().collect().map(|declarations| Program { declarations }).then_ignore(end())
}
