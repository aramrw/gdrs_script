use chumsky::prelude::*;
use crate::ast::*;
use crate::lexer::Token;

type TokenStream<'a> = &'a [Token];

fn type_parser<'a>() -> impl Parser<'a, TokenStream<'a>, Type, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|ty| {
        let base = choice((
            just(Token::ParenOpen).then(just(Token::ParenClose)).to(Type::Unit),
            just(Token::I32).to(Type::I32),
            just(Token::I64).to(Type::I64),
            just(Token::F32).to(Type::F32),
            just(Token::F64).to(Type::F64),
            just(Token::Bool).to(Type::Bool),
            just(Token::Str).to(Type::Str),
            just(Token::StringKw).to(Type::String),
            just(Token::File).to(Type::File),
            just(Token::SelfKw).to(Type::SelfType),
            just(Token::Box).ignore_then(
                ty.clone().delimited_by(just(Token::Lt), just(Token::Gt))
            ).map(|t| Type::BoxPtr(Box::new(t))),
            just(Token::ResultKw).ignore_then(
                ty.clone()
                    .then_ignore(just(Token::Comma))
                    .then(ty.clone())
                    .delimited_by(just(Token::Lt), just(Token::Gt))
            ).map(|(ok, err)| Type::Result(Box::new(ok), Box::new(err))),
            just(Token::ErrorKw).to(Type::Error),
            select! { Token::Ident(name) => Type::Custom(name, Vec::new()) },
        ));

        let reference = just(Token::Amp)
            .ignore_then(just(Token::Mut).or_not())
            .then(ty.clone())
            .map(|(mut_kw, inner)| Type::Ref(Box::new(inner), mut_kw.is_some()));

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

        choice((custom_with_generics, ptr, reference, array, base))
    })
}

fn expr_parser<'a>() -> impl Parser<'a, TokenStream<'a>, Expr, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|expr| {
        let val = choice((
            just(Token::ParenOpen).then(just(Token::ParenClose)).to(Expr::Unit),
            select! { Token::Int(v) => Expr::Int(v) },
            select! { Token::Int64(v) => Expr::Int64(v) },
            select! { Token::Float(v) => Expr::Float(v) },
            select! { Token::Float64(v) => Expr::Float64(v) },
            select! { Token::Boolean(v) => Expr::Bool(v) },
            select! { Token::String(v) => Expr::String(v) },
            select! { Token::Ident(name) => name }
                .then(just(Token::DoubleColon).ignore_then(select! { Token::Ident(name) => name }).repeated().collect::<Vec<_>>())
                .map(|(first, rest)| {
                    let mut full = first;
                    for part in rest {
                        full.push_str("::");
                        full.push_str(&part);
                    }
                    Expr::Variable(full)
                }),
            just(Token::SelfKw).to(Expr::Variable("self".to_string())),
        ));

        let alloc_or_deref = just(Token::Star)
            .ignore_then(choice((
                just(Token::Box).to(AllocKind::Box).then(expr.clone()).map(|(_, e)| Expr::Alloc(Box::new(e), AllocKind::Box)),
                just(Token::Mut).to(AllocKind::RawMut).then(expr.clone()).map(|(_, e)| Expr::Alloc(Box::new(e), AllocKind::RawMut)),
                just(Token::Const).to(AllocKind::RawConst).then(expr.clone()).map(|(_, e)| Expr::Alloc(Box::new(e), AllocKind::RawConst)),
                expr.clone().map(|e| Expr::Deref(Box::new(e))),
            )));

        let array_init = expr.clone()
            .then_ignore(just(Token::Semicolon))
            .then(expr.clone())
            .delimited_by(just(Token::BracketOpen), just(Token::BracketClose))
            .map(|(v, s)| Expr::Call("array_init".to_string(), vec![v, s], None));

        let struct_literal = select! { Token::Ident(name) => name }
            .then(just(Token::DoubleColon).ignore_then(select! { Token::Ident(name) => name }).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                let mut full = first;
                for part in rest {
                    full.push_str("::");
                    full.push_str(&part);
                }
                full
            })
            .or(just(Token::SelfKw).to("self".to_string()))
            .then(
                select! { Token::Ident(name) => name }
                    .then(just(Token::Colon).ignore_then(expr.clone()).or_not())
                    .map(|(name, val)| (name.clone(), val.unwrap_or(Expr::Variable(name))))
                    .separated_by(just(Token::Comma))
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::BraceOpen), just(Token::BraceClose))
            )
            .map(|(name, fields)| Expr::StructLiteral { name, fields, resolved_name: None });

        let borrow = just(Token::Amp)
            .ignore_then(just(Token::Mut).or_not())
            .then(expr.clone())
            .map(|(mut_kw, e)| Expr::Borrow(Box::new(e), mut_kw.is_some()));

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
            .then(just(Token::Bang).or_not())
            .then(expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::ParenOpen), just(Token::ParenClose)))
            .map(|((name, bang), args)| if bang.is_some() { Expr::MacroCall(name, args) } else { Expr::Call(name, args, None) });

        let term = choice((call, borrow, alloc_or_deref, array_init, struct_literal, val, expr.clone().delimited_by(just(Token::ParenOpen), just(Token::ParenClose))));

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
            else if let Some(arguments) = args { Expr::MethodCall(Box::new(lhs), name, arguments, None) }
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

        choice((ptr_decl, var_decl, assign, block, if_stmt, while_stmt, return_stmt, match_stmt, expr_stmt))
    });

    let generic_params = select! { Token::Ident(name) => name }.separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::Lt), just(Token::Gt));

    let param = choice((
        just(Token::Amp).then(just(Token::Mut).or_not()).then(just(Token::SelfKw))
            .map(|((_, mut_kw), _)| Param { 
                name: "self".to_string(), 
                ty: Type::Ref(Box::new(Type::SelfType), mut_kw.is_some()),
                is_mutable: mut_kw.is_some() 
            }),
        just(Token::Mut).or_not()
            .then(select! { Token::Ident(name) => name }.or(just(Token::SelfKw).to("self".to_string())))
            .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
            .map(|((mut_kw, name), ty): ((Option<Token>, String), Option<Type>)| {
                if let Some(t) = ty {
                    Param { name, ty: t, is_mutable: mut_kw.is_some() }
                } else {
                    // If no colon, the 'name' might actually be the type (anonymous param)
                    // This is a bit of a heuristic for RFFI
                    Param { 
                        name: format!("__arg_{}", name), 
                        ty: Type::Custom(name, Vec::new()), 
                        is_mutable: mut_kw.is_some() 
                    }
                }
            }),
        // Direct type as anonymous param
        ty.clone().map(|t| Param { name: "_".to_string(), ty: t, is_mutable: false })
    ));

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

    let method_parser = func_parser.clone().or(extern_func_parser.clone()).then_ignore(just(Token::Semicolon).or_not());

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
        .then(just(Token::Indent).ignore_then(method_parser.clone().repeated().collect()).then_ignore(just(Token::Dedent)))
        .map(|(((_impl_gens, target), _target_gens), functions)| {
            let mut gens = _impl_gens;
            if gens.is_empty() {
                gens = _target_gens;
            }
            ImplDecl { target, generics: gens, functions }
        });

    let extern_impl_parser = just(Token::Extern).ignore_then(just(Token::Impl))
        .ignore_then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then(select! { Token::Ident(name) => name }) // Simpler target for now
        .then(generic_params.clone().or_not().map(|g| g.unwrap_or_default()))
        .then_ignore(just(Token::Colon).or_not())
        .then(just(Token::Indent).ignore_then(method_parser.clone().repeated().collect()).then_ignore(just(Token::Dedent)))
        .map(|(((_impl_gens, target), _target_gens), functions)| ImplDecl { target, generics: _impl_gens, functions });

    let use_parser = just(Token::Use).ignore_then(
        just(Token::Ident("crate".to_string())).then(just(Token::DoubleColon)).or_not()
            .then(
                select! { Token::Ident(name) => name }
                    .separated_by(just(Token::DoubleColon))
                    .at_least(1)
                    .collect::<Vec<_>>()
            )
            .then(
                just(Token::DoubleColon).ignore_then(
                    select! { Token::Ident(name) => name }
                        .separated_by(just(Token::Comma))
                        .collect::<Vec<_>>()
                        .delimited_by(just(Token::BraceOpen), just(Token::BraceClose))
                ).or_not()
            )
    )
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|((is_crate, path), items)| Decl::Use(UseDecl { 
            path, 
            items: items.unwrap_or_default(), 
            is_crate: is_crate.is_some() 
        }));

    let rust_dependency_parser = just(Token::Rust).ignore_then(just(Token::Dependency))
        .ignore_then(select! { Token::Ident(name) => name }.or(select! { Token::String(name) => name }))
        .then_ignore(just(Token::Eq))
        .then(choice((
            select! { Token::String(version) => version },
            just(Token::BraceOpen)
                .ignore_then(any().filter(|t: &Token| t != &Token::BraceClose).repeated().collect::<Vec<_>>())
                .then_ignore(just(Token::BraceClose))
                .map(|tokens| {
                    let mut s = String::from("{ ");
                    for (i, t) in tokens.iter().enumerate() {
                        if i > 0 { s.push(' '); }
                        s.push_str(&t.to_string());
                    }
                    s.push_str(" }");
                    s
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
        obj_parser.map(Decl::Object).then_ignore(just(Token::Semicolon).or_not()), 
        enum_parser.map(Decl::Enum).then_ignore(just(Token::Semicolon).or_not()), 
        impl_parser.map(Decl::Impl).then_ignore(just(Token::Semicolon).or_not()), 
        extern_func_parser.map(Decl::ExternFunction).then_ignore(just(Token::Semicolon).or_not()), 
        extern_obj_parser.map(Decl::ExternObject).then_ignore(just(Token::Semicolon).or_not()), 
        extern_enum_parser.map(Decl::ExternEnum).then_ignore(just(Token::Semicolon).or_not()),
        extern_impl_parser.map(Decl::ExternImpl).then_ignore(just(Token::Semicolon).or_not()),
        use_parser
    ));

    decl.repeated().collect().map(|declarations| Program { declarations }).then_ignore(end())
}
