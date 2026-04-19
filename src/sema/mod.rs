// src/sema/mod.rs

mod types;
mod analysis; // This imports the analysis module, which will handle its own submodules.

// Re-export necessary items for external use if needed
pub use types::TypeInfo;
pub use analysis::AnalysisInfo;
use analysis::statements::StatementAnalyzer;

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
                    
                    let old_gens = self.type_info.generic_params.clone();
                    for (name, bounds) in &func.generics {
                        self.type_info.generic_params.insert(name.clone(), bounds.clone());
                    }

                    // Add parameters to symbols
                    for param in &func.params {
                        let ty = self.type_info.resolve_type(&param.ty, prefix);
                        self.analysis_info.symbols.insert(param.name.clone(), (ty, param.is_mutable));
                    }
                    let mut stmt_analyzer = StatementAnalyzer::new(&mut self.analysis_info, &mut self.type_info);
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
                        // Add parameters to symbols
                        for param in &func.params {
                            let ty = if param.name == "self" {
                                // Resolve self type
                                if full_target.contains('<') {
                                    if full_target.ends_with("<>") {
                                        Type::Custom(full_target.clone(), Vec::new())
                                    } else {
                                        let parts: Vec<_> = full_target.split('<').collect();
                                        let name = parts[0].to_string();
                                        Type::Custom(format!("{}<>", name), Vec::new())
                                    }
                                } else {
                                    Type::Custom(full_target.clone(), Vec::new())
                                }
                            } else {
                                self.type_info.resolve_type(&param.ty, prefix)
                            };
                            self.analysis_info.symbols.insert(param.name.clone(), (ty, param.is_mutable));
                        }
                        let mut stmt_analyzer = StatementAnalyzer::new(&mut self.analysis_info, &mut self.type_info);
                        stmt_analyzer.analyze_stmt(&mut func.body)?;
                    }
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
