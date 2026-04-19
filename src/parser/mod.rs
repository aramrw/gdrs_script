mod expr;
mod types;
mod stmt;

use crate::ast::*;
use crate::lexer::{Span, Token};
use crate::parser::expr::expr_parser;
use crate::parser::stmt::stmt_parser;
use crate::parser::types::type_parser;
use chumsky::input::ValueInput;
use chumsky::prelude::*;


pub type ParserExtra<'a> = extra::Err<Rich<'a, Token, Span>>;

// =========================================================================
// Extension Traits to Eliminate Boilerplate
// =========================================================================

pub trait ParserExt<'a, I, O>: Parser<'a, I, O, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    fn parens(self) -> impl Parser<'a, I, O, ParserExtra<'a>> + Clone {
        self.delimited_by(just(Token::ParenOpen), just(Token::ParenClose))
    }

    fn brackets(self) -> impl Parser<'a, I, O, ParserExtra<'a>> + Clone {
        self.delimited_by(just(Token::BracketOpen), just(Token::BracketClose))
    }

    fn braces(self) -> impl Parser<'a, I, O, ParserExtra<'a>> + Clone {
        self.delimited_by(just(Token::BraceOpen), just(Token::BraceClose))
    }
}
impl<'a, I, O, T> ParserExt<'a, I, O> for T
where
    I: ValueInput<'a, Token = Token, Span = Span>,
    T: Parser<'a, I, O, ParserExtra<'a>> + Clone,
{
}

pub trait ExprParserExt<'a, I>: Parser<'a, I, ExprKind, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
    Self: 'a,
{
    fn into_expr(self) -> Boxed<'a, 'a, I, Expr, ParserExtra<'a>> {
        self.map_with(|kind, e| Expr {
            kind,
            span: e.span(),
            ty: None,
        }).boxed()
    }
}
impl<'a, I, T> ExprParserExt<'a, I> for T
where
    I: ValueInput<'a, Token = Token, Span = Span>,
    T: Parser<'a, I, ExprKind, ParserExtra<'a>> + Clone + 'a,
{
}

pub trait StmtParserExt<'a, I>: Parser<'a, I, StmtKind, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
    Self: 'a,
{
    fn into_stmt(self) -> Boxed<'a, 'a, I, Stmt, ParserExtra<'a>> {
        self.map_with(|kind, e| Stmt {
            kind,
            span: e.span(),
        }).boxed()
    }
}
impl<'a, I, T> StmtParserExt<'a, I> for T
where
    I: ValueInput<'a, Token = Token, Span = Span>,
    T: Parser<'a, I, StmtKind, ParserExtra<'a>> + Clone + 'a,
{
}

// =========================================================================
// Reusable Sub-Parsers
// =========================================================================

fn ident<'a, I>() -> impl Parser<'a, I, String, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    select! { Token::Ident(name) => name }
}

fn string_lit<'a, I>() -> impl Parser<'a, I, String, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    select! { Token::String(s) => s }
}

fn double_colon_path<'a, I>() -> impl Parser<'a, I, String, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    ident()
        .then(
            just(Token::DoubleColon)
                .ignore_then(ident())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
            let mut full = first;
            for part in rest {
                full.push_str("::");
                full.push_str(&part);
            }
            full
        })
}

fn generic_params_parser<'a, I>()
-> impl Parser<'a, I, Vec<(String, Vec<String>)>, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    ident()
        .then(
            just(Token::Colon)
                .ignore_then(ident().separated_by(just(Token::Plus)).collect::<Vec<_>>())
                .or_not()
                .map(|b| b.unwrap_or_default()),
        )
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::Lt), just(Token::Gt))
}

fn attribute_parser<'a, I>() -> impl Parser<'a, I, Vec<String>, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    select! { Token::Attribute(s) => s }
        .repeated()
        .collect::<Vec<_>>()
}

fn rust_path_parser<'a, I>() -> impl Parser<'a, I, String, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Eq).ignore_then(string_lit())
}

// =========================================================================
// Types, Expressions, and Statements
// =========================================================================




fn block_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Indent)
        .ignore_then(stmt.repeated().collect())
        .then_ignore(just(Token::Dedent))
        .map(StmtKind::Block)
        .into_stmt()
}

// =========================================================================
// Declarations
// =========================================================================

struct FuncSig {
    is_async: bool,
    name: String,
    generics: Vec<(String, Vec<String>)>,
    params: Vec<Param>,
    return_type: Option<Type>,
    rust_path: Option<String>,
}

fn param_parser<'a, I>() -> impl Parser<'a, I, Param, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    let ty = type_parser::<I>();
    choice((
        just(Token::Amp)
            .then(just(Token::Mut).or_not())
            .then(just(Token::SelfKw))
            .map(|((_, mut_kw), _)| Param {
                name: "self".to_string(),
                ty: Type::Ref(Box::new(Type::SelfType), mut_kw.is_some()),
                is_mutable: mut_kw.is_some(),
            }),
        just(Token::Mut)
            .or_not()
            .then(ident().or(just(Token::SelfKw).to("self".to_string())))
            .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
            .map(|((mut_kw, name), ty)| {
                if let Some(t) = ty {
                    Param {
                        name,
                        ty: t,
                        is_mutable: mut_kw.is_some(),
                    }
                } else {
                    Param {
                        name: format!("__arg_{}", name),
                        ty: Type::Custom(name, Vec::new()),
                        is_mutable: mut_kw.is_some(),
                    }
                }
            }),
        ty.map(|t| Param {
            name: "_".to_string(),
            ty: t,
            is_mutable: false,
        }),
    ))
}

fn func_sig_parser<'a, I>() -> impl Parser<'a, I, FuncSig, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Async)
        .or_not()
        .map(|a| a.is_some())
        .then_ignore(just(Token::Fn))
        .then(ident())
        .then(
            generic_params_parser()
                .or_not()
                .map(|g| g.unwrap_or_default()),
        )
        .then(
            param_parser()
                .separated_by(just(Token::Comma))
                .collect::<Vec<_>>()
                .parens(),
        )
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then(rust_path_parser().or_not())
        .map(
            |(((((is_async, name), generics), params), return_type), rust_path)| FuncSig {
                is_async,
                name,
                generics,
                params,
                return_type,
                rust_path,
            },
        )
}

fn func_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, Function, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    attribute_parser()
        .then(func_sig_parser())
        .then_ignore(just(Token::Colon).or_not())
        .then(block_parser(stmt))
        .map(|((attributes, sig), body)| Function {
            name: sig.name,
            generics: sig.generics,
            params: sig.params,
            return_type: sig.return_type,
            body,
            is_async: sig.is_async,
            rust_path: sig.rust_path,
            attributes,
        })
}

fn extern_func_parser<'a, I>() -> impl Parser<'a, I, Function, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    attribute_parser()
        .then(just(Token::Extern).ignore_then(func_sig_parser()))
        .map_with(|(attributes, sig), e| Function {
            name: sig.name,
            generics: sig.generics,
            params: sig.params,
            return_type: sig.return_type,
            is_async: sig.is_async,
            rust_path: sig.rust_path,
            attributes,
            body: Stmt {
                kind: StmtKind::Block(Vec::new()),
                span: e.span(),
            },
        })
}

fn obj_parser<'a, I>() -> impl Parser<'a, I, ObjectDecl, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    let field = attribute_parser()
        .then(ident())
        .then_ignore(just(Token::Colon))
        .then(type_parser())
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|((attributes, name), ty)| Field {
            name,
            ty,
            attributes,
        });

    attribute_parser()
        .then(
            just(Token::Obj)
                .ignore_then(ident())
                .then(
                    generic_params_parser()
                        .or_not()
                        .map(|g| g.unwrap_or_default()),
                )
                .then(rust_path_parser().or_not())
                .then_ignore(just(Token::Colon).or_not())
                .then(
                    just(Token::Indent)
                        .ignore_then(field.repeated().collect())
                        .then_ignore(just(Token::Dedent))
                        .or_not(),
                ),
        )
        .map(
            |(attributes, (((name, generics), rust_path), fields))| ObjectDecl {
                name,
                generics,
                fields: fields.unwrap_or_default(),
                rust_path,
                attributes,
            },
        )
}

fn extern_obj_parser<'a, I>() -> impl Parser<'a, I, ObjectDecl, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Extern).ignore_then(obj_parser())
}

fn enum_parser<'a, I>() -> impl Parser<'a, I, EnumDecl, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    let variant = attribute_parser()
        .then(ident())
        .then(
            type_parser()
                .separated_by(just(Token::Comma))
                .collect::<Vec<_>>()
                .parens()
                .or_not(),
        )
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|((attributes, name), types)| Variant {
            name,
            types: types.unwrap_or_default(),
            attributes,
        });

    attribute_parser()
        .then(
            just(Token::Enum)
                .ignore_then(ident())
                .then(
                    generic_params_parser()
                        .or_not()
                        .map(|g| g.unwrap_or_default()),
                )
                .then(rust_path_parser().or_not())
                .then_ignore(just(Token::Colon).or_not())
                .then(
                    just(Token::Indent)
                        .ignore_then(variant.repeated().collect())
                        .then_ignore(just(Token::Dedent)),
                ),
        )
        .map(
            |(attributes, (((name, generics), rust_path), variants))| EnumDecl {
                name,
                generics,
                variants,
                rust_path,
                attributes,
            },
        )
}

fn extern_enum_parser<'a, I>() -> impl Parser<'a, I, EnumDecl, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Extern).ignore_then(enum_parser())
}

fn trait_method_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, Function, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    attribute_parser()
        .then(func_sig_parser())
        .then(just(Token::Colon).ignore_then(block_parser(stmt)).or_not())
        .map_with(|((attributes, sig), body), e| Function {
            name: sig.name,
            generics: sig.generics,
            params: sig.params,
            return_type: sig.return_type,
            body: body.unwrap_or(Stmt {
                kind: StmtKind::Block(Vec::new()),
                span: e.span(),
            }),
            is_async: sig.is_async,
            rust_path: sig.rust_path,
            attributes,
        })
}

fn trait_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, TraitDecl, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    attribute_parser()
        .then_ignore(just(Token::Trait))
        .then(ident())
        .then(
            generic_params_parser()
                .or_not()
                .map(|g| g.unwrap_or_default()),
        )
        .then(
            just(Token::Colon)
                .ignore_then(ident().separated_by(just(Token::Plus)).collect::<Vec<_>>())
                .or_not()
                .map(|b| b.unwrap_or_default()),
        )
        .then_ignore(just(Token::Colon).or_not())
        .then(
            just(Token::Indent)
                .ignore_then(
                    trait_method_parser(stmt)
                        .then_ignore(just(Token::Semicolon).or_not())
                        .repeated()
                        .collect(),
                )
                .then_ignore(just(Token::Dedent)),
        )
        .map(
            |((((attributes, name), generics), bounds), functions)| TraitDecl {
                name,
                generics,
                bounds,
                functions,
                attributes,
            },
        )
}

fn extern_trait_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, TraitDecl, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Extern).ignore_then(trait_parser(stmt))
}

fn impl_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, ImplDecl, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    let method = func_parser(stmt.clone())
        .or(extern_func_parser())
        .then_ignore(just(Token::Semicolon).or_not());

    just(Token::Impl)
        .ignore_then(
            generic_params_parser()
                .or_not()
                .map(|g| g.unwrap_or_default()),
        )
        .then(double_colon_path())
        .then(just(Token::For).ignore_then(ident()).or_not())
        .then(
            generic_params_parser()
                .or_not()
                .map(|g| g.unwrap_or_default()),
        )
        .then_ignore(just(Token::Colon).or_not())
        .then(
            just(Token::Indent)
                .ignore_then(method.repeated().collect())
                .then_ignore(just(Token::Dedent)),
        )
        .map(
            |((((impl_gens, trait_or_target), target), target_gens), functions)| {
                let gens = if impl_gens.is_empty() {
                    target_gens
                } else {
                    impl_gens
                };
                if let Some(target_name) = target {
                    ImplDecl {
                        trait_name: Some(trait_or_target),
                        target: target_name,
                        generics: gens,
                        functions,
                    }
                } else {
                    ImplDecl {
                        trait_name: None,
                        target: trait_or_target,
                        generics: gens,
                        functions,
                    }
                }
            },
        )
}

fn extern_impl_parser<'a, I>(
    stmt: impl Parser<'a, I, Stmt, ParserExtra<'a>> + Clone + 'a,
) -> impl Parser<'a, I, ImplDecl, ParserExtra<'a>> + Clone + 'a
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    let method = func_parser(stmt.clone())
        .or(extern_func_parser())
        .then_ignore(just(Token::Semicolon).or_not());

    just(Token::Extern)
        .ignore_then(just(Token::Impl))
        .ignore_then(
            generic_params_parser()
                .or_not()
                .map(|g| g.unwrap_or_default()),
        )
        .then(ident())
        .then(
            generic_params_parser()
                .or_not()
                .map(|g| g.unwrap_or_default()),
        )
        .then_ignore(just(Token::Colon).or_not())
        .then(
            just(Token::Indent)
                .ignore_then(method.repeated().collect())
                .then_ignore(just(Token::Dedent)),
        )
        .map(
            |(((_impl_gens, target), _target_gens), functions)| ImplDecl {
                trait_name: None,
                target,
                generics: _impl_gens,
                functions,
            },
        )
}

fn use_parser<'a, I>() -> impl Parser<'a, I, Decl, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Use)
        .ignore_then(
            just(Token::Ident("crate".to_string()))
                .then(just(Token::DoubleColon))
                .or_not()
                .then(
                    ident()
                        .separated_by(just(Token::DoubleColon))
                        .at_least(1)
                        .collect::<Vec<_>>(),
                )
                .then(choice((
                    just(Token::DoubleColon)
                        .ignore_then(just(Token::Star))
                        .to((Vec::new(), true)),
                    just(Token::DoubleColon)
                        .ignore_then(
                            ident()
                                .separated_by(just(Token::Comma))
                                .collect::<Vec<_>>()
                                .braces(),
                        )
                        .map(|items| (items, false)),
                    empty().to((Vec::new(), false)),
                ))),
        )
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|((is_crate, path), (items, is_wildcard))| {
            Decl::Use(UseDecl {
                path,
                items,
                is_wildcard,
                is_crate: is_crate.is_some(),
            })
        })
}

fn rust_dependency_parser<'a, I>() -> impl Parser<'a, I, Decl, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Rust)
        .ignore_then(just(Token::Dependency))
        .ignore_then(ident().or(string_lit()))
        .then_ignore(just(Token::Eq))
        .then(choice((
            string_lit(),
            just(Token::BraceOpen)
                .ignore_then(
                    any()
                        .filter(|tok: &Token| tok != &Token::BraceClose)
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(just(Token::BraceClose))
                .map(|tokens: Vec<Token>| {
                    let mut s = String::from("{ ");
                    for (i, t) in tokens.iter().enumerate() {
                        if i > 0 {
                            s.push(' ');
                        }
                        s.push_str(&t.to_string());
                    }
                    s.push_str(" }");
                    s
                }),
        )))
        .then_ignore(just(Token::Semicolon).or_not())
        .map(|(name, version)| Decl::RustDependency(name, version))
}

fn rust_block_parser<'a, I>() -> impl Parser<'a, I, Decl, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    just(Token::Rust)
        .ignore_then(just(Token::Colon))
        .ignore_then(just(Token::Indent))
        .ignore_then(any().repeated().collect::<Vec<_>>())
        .then_ignore(just(Token::Dedent))
        .map(|_tokens| {
            Decl::RustBlock(
                "// Raw rust code injection not fully implemented in parser yet".to_string(),
            )
        })
}

// =========================================================================
// Main Program Parser
// =========================================================================

pub fn parser<'a, I>() -> impl Parser<'a, I, Program, ParserExtra<'a>>
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    recursive(|program| {
        let stmt = recursive(|stmt| {
            let expr = recursive(|expr| expr_parser(stmt.clone(), expr.clone()));
            stmt_parser(stmt, expr)
        });
        let expr = recursive(|expr| expr_parser(stmt.clone(), expr.clone()));

        let decl = choice((
            rust_dependency_parser(),
            rust_block_parser(),
            func_parser(stmt.clone()).map(Decl::Function),
            obj_parser()
                .map(Decl::Object)
                .then_ignore(just(Token::Semicolon).or_not()),
            enum_parser()
                .map(Decl::Enum)
                .then_ignore(just(Token::Semicolon).or_not()),
            trait_parser(stmt.clone())
                .map(Decl::Trait)
                .then_ignore(just(Token::Semicolon).or_not()),
            impl_parser(stmt.clone())
                .map(Decl::Impl)
                .then_ignore(just(Token::Semicolon).or_not()),
            extern_func_parser()
                .map(Decl::ExternFunction)
                .then_ignore(just(Token::Semicolon).or_not()),
            extern_obj_parser()
                .map(Decl::ExternObject)
                .then_ignore(just(Token::Semicolon).or_not()),
            extern_enum_parser()
                .map(Decl::ExternEnum)
                .then_ignore(just(Token::Semicolon).or_not()),
            extern_trait_parser(stmt.clone())
                .map(Decl::ExternTrait)
                .then_ignore(just(Token::Semicolon).or_not()),
            extern_impl_parser(stmt.clone())
                .map(Decl::ExternImpl)
                .then_ignore(just(Token::Semicolon).or_not()),
            use_parser(),
        ));

        decl.repeated()
            .collect()
            .map(|declarations| Program { declarations })
            .then_ignore(end())
    })
}
