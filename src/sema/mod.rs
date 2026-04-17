// src/sema/mod.rs

mod types;
mod analysis; // This imports the analysis module, which will handle its own submodules.

// Re-export necessary items for external use if needed
pub use types::TypeInfo;
pub use analysis::AnalysisInfo;

use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;

// --- Combined SemanticAnalyzer struct ---
// It now holds instances of the specialized structs for type info and analysis logic.
#[derive(Debug)]
pub struct SemanticAnalyzer {
    type_info: types::TypeInfo,
    analysis_info: analysis::AnalysisInfo,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            type_info: types::TypeInfo::new(),
            analysis_info: analysis::AnalysisInfo::new(),
        }
    }

    // The main analysis entry point. It orchestrates the process.
    pub fn analyze(&mut self, program: &mut Program) -> Result<(), CompilerError> {
        // 1. Collect all type information first (objects, enums, traits, functions, etc.)
        self.type_info.collect_decls(&program.declarations, "")?;
        self.type_info.collect_impls(&program.declarations, "")?;

        // 2. Perform semantic analysis on declarations, statements, and expressions
        self.analyze_decls(&mut program.declarations, "")?;
        
        Ok(())
    }

    // Orchestrates the analysis of declarations (functions, impl blocks, modules)
    // This method will call into analysis_info.analyze_function which uses specialized analyzers.
    fn analyze_decls(&mut self, decls: &mut [Decl], prefix: &str) -> Result<(), CompilerError> {
        // Set up the current prefix for analysis context
        self.analysis_info.current_prefix = prefix.to_string();
        
        for decl in decls {
            match decl {
                Decl::Function(func) => {
                    // Analyze function body. This requires setting up symbol tables etc.
                    // analyze_function is now in analysis.rs and needs type_info passed.
                    self.analysis_info.analyze_function(func, None, None, &mut self.type_info)?;
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
                        // analyze_function needs to know about 'self' and its type,
                        // and the impl block's generics.
                        self.analysis_info.analyze_function(func, Some(&full_target), Some(&imp.generics), &mut self.type_info)?;
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
                }
                _ => {} // Ignore other declaration types for now
            }
        }
        Ok(())
    }
}
