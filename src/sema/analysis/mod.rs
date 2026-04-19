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

    pub fn path_to_string(&self, path: &[crate::ast::PathPart], type_info: &crate::sema::types::TypeInfo) -> String {
        let mut full_name = String::new();
        for (i, p) in path.iter().enumerate() {
            if i == 0 {
                if let Some(alias) = type_info.aliases.get(&p.name) {
                    full_name = alias.clone();
                } else {
                    full_name = p.name.clone();
                }
            } else {
                full_name.push_str("::");
                full_name.push_str(&p.name);
            }
        }
        full_name
    }

    pub fn promote_variable(&mut self, _name: &str, _is_await: bool) {
        // Implement logic
    }

    pub fn types_equal(&self, a: &crate::ast::Type, b: &crate::ast::Type) -> bool {
        match (a, b) {
            (crate::ast::Type::Any, _) | (_, crate::ast::Type::Any) => true,
            (crate::ast::Type::Generic(_), _) | (_, crate::ast::Type::Generic(_)) => true,
            (crate::ast::Type::Ref(a_inner, a_mut), crate::ast::Type::Ref(b_inner, b_mut)) => {
                *a_mut == *b_mut && self.types_equal(a_inner, b_inner)
            }
            (crate::ast::Type::BoxPtr(a_inner), crate::ast::Type::BoxPtr(b_inner)) => {
                self.types_equal(a_inner, b_inner)
            }
            (crate::ast::Type::Custom(a_name, a_gens), crate::ast::Type::Custom(b_name, b_gens)) => {
                if a_name != b_name {
                    return false;
                }
                if a_gens.len() != b_gens.len() {
                    return false;
                }
                for (ag, bg) in a_gens.iter().zip(b_gens.iter()) {
                    if !self.types_equal(ag, bg) {
                        return false;
                    }
                }
                true
            }
            _ => a == b,
        }
    }

    pub fn deref_type(&self, ty: &crate::ast::Type) -> crate::ast::Type {
        match ty {
            crate::ast::Type::Ref(inner, _)
            | crate::ast::Type::BoxPtr(inner)
            | crate::ast::Type::RawPtr(inner, _)
            | crate::ast::Type::Managed(inner)
            | crate::ast::Type::ThreadSafe(inner) => self.deref_type(inner),
            _ => ty.clone(),
        }
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
