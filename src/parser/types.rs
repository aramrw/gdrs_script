use crate::ast::*;
use crate::lexer::{Span, Token};
use crate::parser::{ParserExt, ParserExtra, double_colon_path, ident};
use chumsky::input::ValueInput;
use chumsky::prelude::*;

pub fn type_parser<'a, I>() -> impl Parser<'a, I, Type, ParserExtra<'a>> + Clone
where
    I: ValueInput<'a, Token = Token, Span = Span>,
{
    recursive(|ty| {
        let base = choice((
            just(Token::ParenOpen)
                .then(just(Token::ParenClose))
                .to(Type::Unit),
            just(Token::I32).to(Type::I32),
            just(Token::I64).to(Type::I64),
            just(Token::F32).to(Type::F32),
            just(Token::F64).to(Type::F64),
            just(Token::Bool).to(Type::Bool),
            just(Token::Str).to(Type::Str),
            just(Token::SelfKw).to(Type::SelfType),
            just(Token::Box)
                .ignore_then(ty.clone().delimited_by(just(Token::Lt), just(Token::Gt)))
                .map(|t| Type::BoxPtr(Box::new(t))),
            just(Token::ResultKw)
                .ignore_then(
                    ty.clone()
                        .then_ignore(just(Token::Comma))
                        .then(ty.clone())
                        .delimited_by(just(Token::Lt), just(Token::Gt)),
                )
                .map(|(ok, err)| Type::Result(Box::new(ok), Box::new(err))),
            just(Token::ErrorKw).to(Type::Error),
            ident().map(|name| Type::Custom(name, Vec::new())),
        ));

        let reference = just(Token::Amp)
            .ignore_then(just(Token::Mut).or_not())
            .then(ty.clone())
            .map(|(mut_kw, inner)| Type::Ref(Box::new(inner), mut_kw.is_some()));

        let ptr = choice((
            // Weak pointers: ~*T or ~**T
            just(Token::Tilde)
                .ignore_then(just(Token::Star))
                .ignore_then(choice((
                    just(Token::Star)
                        .ignore_then(ty.clone())
                        .map(|inner| Type::WeakThreadSafe(Box::new(inner))),
                    ty.clone().map(|inner| Type::WeakManaged(Box::new(inner))),
                ))),
            // Pointers starting with *: *T, **T, *mut T, *const T, *box T
            just(Token::Star).ignore_then(choice((
                just(Token::Box)
                    .ignore_then(ty.clone())
                    .map(|inner| Type::BoxPtr(Box::new(inner))),
                just(Token::Mut)
                    .ignore_then(ty.clone())
                    .map(|inner| Type::RawPtr(Box::new(inner), true)),
                just(Token::Const)
                    .ignore_then(ty.clone())
                    .map(|inner| Type::RawPtr(Box::new(inner), false)),
                just(Token::Star)
                    .ignore_then(ty.clone())
                    .map(|inner| Type::ThreadSafe(Box::new(inner))),
                ty.clone().map(|inner| Type::Managed(Box::new(inner))),
            ))),
        ));

        let array = ty
            .clone()
            .then_ignore(just(Token::Semicolon))
            .then(select! { Token::Int(v) => v as usize })
            .brackets()
            .map(|(t, s)| Type::Array(Box::new(t), s));

        let custom_with_generics = double_colon_path()
            .then(
                ty.separated_by(just(Token::Comma))
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::Lt), just(Token::Gt))
                    .or_not(),
            )
            .map(|(name, gens)| Type::Custom(name, gens.unwrap_or_default()));

        choice((custom_with_generics, ptr, reference, array, base))
    })
}
