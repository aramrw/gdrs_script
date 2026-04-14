use miette::{Diagnostic, SourceSpan};
use thiserror::Error;
use crate::lexer::{Token, Span};
use chumsky::error::Rich;

#[derive(Error, Diagnostic, Debug, Clone, PartialEq)]
pub enum LexError {
    #[error("Invalid indentation")]
    #[diagnostic(code(lexer::invalid_indentation), help("Check your indentation levels."))]
    InvalidIndentation {
        #[label("this indentation level is inconsistent")]
        span: SourceSpan,
    },

    #[error("Unexpected character '{character}'")]
    #[diagnostic(code(lexer::unexpected_character))]
    UnexpectedCharacter {
        character: char,
        #[label("unexpected character")]
        span: SourceSpan,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, PartialEq)]
pub enum CompilerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Lex(#[from] LexError),

    #[error("Parse error: {message}")]
    #[diagnostic(code(parser::error))]
    Parse {
        message: String,
        #[label("{message}")]
        span: SourceSpan,
    },

    #[error("Semantic error: {message}")]
    #[diagnostic(code(sema::error))]
    Semantic {
        message: String,
        #[label("{message}")]
        span: SourceSpan,
    },
}

impl CompilerError {
    pub fn from_rich(err: Rich<Token, Span>) -> Self {
        let span = SourceSpan::new(err.span().start.into(), (err.span().end - err.span().start).into());
        let message = format!("{}", err);
        CompilerError::Parse { message, span }
    }
}
