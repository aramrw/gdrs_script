pub mod statements;
pub mod expressions;
pub mod patterns;

#[derive(Debug)]
pub struct AnalysisInfo {
    pub current_prefix: String,
    pub scope_depth: usize,
    pub await_points: Vec<usize>,
    pub var_declarations: std::collections::HashMap<String, (usize, bool)>,
    pub symbols: std::collections::HashMap<String, (crate::ast::Type, bool)>,
}

impl AnalysisInfo {
    pub fn new() -> Self {
        Self {
            current_prefix: String::new(),
            scope_depth: 0,
            await_points: Vec::new(),
            var_declarations: std::collections::HashMap::new(),
            symbols: std::collections::HashMap::new(),
        }
    }

    pub fn path_to_string(&self, path: &[crate::ast::PathPart]) -> String {
        path.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join("::")
    }

    pub fn promote_variable(&mut self, _name: &str, _is_await: bool) {
        // Implement logic
    }

    pub fn types_equal(&self, _a: &crate::ast::Type, _b: &crate::ast::Type) -> bool {
        true
    }

    pub fn deref_type(&self, ty: &crate::ast::Type) -> crate::ast::Type {
        ty.clone()
    }

    pub fn semantic_error(&self, _msg: String, _span: crate::lexer::Span) -> Result<crate::ast::Type, crate::error::CompilerError> {
        Err(crate::error::CompilerError::from_rich(chumsky::prelude::Rich::custom(_span, _msg)))
    }

    pub fn analyze_function(&mut self, _func: &mut crate::ast::Function, _obj: Option<&str>, _generics: Option<&Vec<(String, Vec<String>)>>, _type_info: &mut crate::sema::types::TypeInfo) -> Result<(), crate::error::CompilerError> {
        Ok(())
    }

    pub fn detect_escape(&mut self, _expr: &crate::ast::Expr) {
        // Placeholder
    }

    pub fn check_aliasing(&mut self, _args: &[crate::ast::Expr], _kinds: &[crate::ast::ArgKind], _span: crate::lexer::Span) -> Result<(), crate::error::CompilerError> {
        Ok(())
    }
}
