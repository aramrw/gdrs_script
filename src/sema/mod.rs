// src/sema/mod.rs

mod analysis;
mod types; // This imports the analysis module, which will handle its own submodules.

// Re-export necessary items for external use if needed
pub use analysis::AnalysisInfo;
use analysis::expressions::ExpressionAnalyzer;
use analysis::statements::StatementAnalyzer;
pub use types::TypeInfo;

use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;
use std::collections::HashSet;

pub struct SemanticAnalyzer {
    pub type_info: TypeInfo,
    pub analysis_info: AnalysisInfo,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            type_info: TypeInfo::new(),
            analysis_info: AnalysisInfo::new(),
        }
    }

    pub fn analyze(&mut self, program: &mut Program) -> Result<(), CompilerError> {
        self.type_info.collect_decls(&program.declarations, "")?;
        self.type_info.collect_impls(&program.declarations, "")?;
        self.analyze_decls(&mut program.declarations, "")
    }

    fn analyze_decls(&mut self, decls: &mut [Decl], prefix: &str) -> Result<(), CompilerError> {
        self.analysis_info.current_prefix = prefix.to_string();
        for decl in decls {
            match decl {
                Decl::Function(func) => {
                    self.analysis_info.symbols.clear();
                    self.analysis_info.await_points.clear();
                    self.analysis_info.var_declarations.clear();
                    self.analysis_info.scope_depth = 0;

                    let old_gens = self.type_info.generic_params.clone();
                    for (name, bounds) in &func.generics {
                        self.type_info
                            .generic_params
                            .insert(name.clone(), bounds.clone());
                    }

                    // Add parameters to symbols
                    for param in &func.params {
                        let ty = self.type_info.resolve_type(&param.ty, prefix);
                        self.analysis_info
                            .symbols
                            .insert(param.name.clone(), (ty, param.is_mutable));
                    }
                    let mut stmt_analyzer =
                        StatementAnalyzer::new(&mut self.analysis_info, &mut self.type_info);
                    stmt_analyzer.analyze_stmt(&mut func.body)?;

                    self.type_info.generic_params = old_gens;
                }
                Decl::Impl(imp) => {
                    let mut full_target = if prefix.is_empty() {
                        imp.target.clone()
                    } else {
                        if imp.target.contains("::") {
                            imp.target.clone()
                        } else {
                            format!("{}::{}", prefix, imp.target)
                        }
                    };

                    // Handle generic impl targets (e.g., `impl<T> MyObj<T>`)
                    if !imp.generics.is_empty() && !full_target.ends_with("<>") {
                        full_target = format!("{}<>", full_target);
                    }

                    // Analyze methods within the impl block
                    for func in &mut imp.functions {
                        self.analysis_info.symbols.clear();
                        self.analysis_info.await_points.clear();
                        self.analysis_info.var_declarations.clear();
                        self.analysis_info.scope_depth = 0;

                        // 1. Extract the actual target struct name (handles "Sub for Vec2" -> "Vec2")
                        let target_name =
                            if let Some((_, type_part)) = full_target.split_once(" for ") {
                                type_part.trim()
                            } else {
                                full_target.as_str()
                            };

                        // Build the base target type
                        let base_self_ty = if target_name.contains('<') {
                            if target_name.ends_with("<>") {
                                Type::Custom(target_name.to_string(), Vec::new())
                            } else {
                                let name = target_name.split('<').next().unwrap();
                                Type::Custom(format!("{}<>", name), Vec::new())
                            }
                        } else {
                            Type::Custom(target_name.to_string(), Vec::new())
                        };

                        // Add parameters to symbols
                        for param in &func.params {
                            let ty = if param.name == "self" {
                                // 2. Preserve reference wrappers (&self, &mut self, self)
                                match &param.ty {
                                    Type::Ref(_, is_mut) => {
                                        Type::Ref(Box::new(base_self_ty.clone()), *is_mut)
                                    }
                                    _ => base_self_ty.clone(),
                                }
                            } else {
                                self.type_info.resolve_type(&param.ty, prefix)
                            };

                            self.analysis_info
                                .symbols
                                .insert(param.name.clone(), (ty, param.is_mutable));
                        }

                        let mut stmt_analyzer =
                            StatementAnalyzer::new(&mut self.analysis_info, &mut self.type_info);
                        stmt_analyzer.analyze_stmt(&mut func.body)?;
                    }
                }
                Decl::Const(c) => {
                    let mut expr_analyzer =
                        ExpressionAnalyzer::new(&mut self.analysis_info, &mut self.type_info);
                    let val_ty = expr_analyzer.analyze_expr(&mut c.value)?;
                    let expected_ty =
                        c.ty.as_ref()
                            .map(|t| self.type_info.resolve_type(t, prefix));
                    if let Some(et) = expected_ty {
                        if !self.analysis_info.types_equal(&et, &val_ty) {
                            return Err(CompilerError::from_rich(chumsky::prelude::Rich::custom(
                                c.span,
                                format!(
                                    "Type mismatch in constant '{}': expected {:?}, found {:?}",
                                    c.name, et, val_ty
                                ),
                            )));
                        }
                    }
                    // Update type in constants map if it was Any
                    let full_name = if prefix.is_empty() {
                        c.name.clone()
                    } else {
                        format!("{}::{}", prefix, c.name)
                    };
                    self.type_info.constants.insert(full_name, val_ty);
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{}::{}", prefix, name)
                    };
                    // Recursively analyze declarations in submodules
                    self.analyze_decls(inner, &new_prefix)?;
                    // Restore prefix
                    self.analysis_info.current_prefix = prefix.to_string();
                }
                _ => {} // Ignore other declaration types for now
            }
        }
        Ok(())
    }
}
