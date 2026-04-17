
// src/sema/patterns.rs

use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;
use std::collections::HashMap; // Required for context if called standalone

// Assuming AnalysisInfo and TypeInfo are accessible or passed as context
// For standalone compilation, these would need to be available.
// In the final structure, they will be passed or accessed via a shared context.

pub struct PatternAnalyzer<'a> {
    analysis_info: &'a mut analysis::AnalysisInfo,
    type_info: &'a mut types::TypeInfo,
}

impl<'a> PatternAnalyzer<'a> {
    pub fn new(
        analysis_info: &'a mut analysis::AnalysisInfo,
        type_info: &'a mut types::TypeInfo,
    ) -> Self {
        Self { analysis_info, type_info }
    }

    pub fn analyze_pattern(
        &mut self,
        pattern: &mut Pattern,
        expr_ty: &Type,
        span: Span,
    ) -> Result<(), CompilerError> {
        match pattern {
            Pattern::Variant(enum_name, variant_name, params) => {
                // Handle Result enum explicitly for Ok and Err patterns
                if enum_name.is_empty() { // If enum_name is not specified, try to infer from expr_ty
                    match expr_ty {
                        Type::Result(ok, err) => {
                            if variant_name == "Ok" {
                                *enum_name = "Result".into(); // Set enum name for clarity
                                if params.len() == 1 {
                                    // Bind the parameter to the Ok type
                                    self.analysis_info.symbols.insert(params[0].clone(), (*ok.clone(), false));
                                }
                                return Ok(());
                            } else if variant_name == "Err" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    // Bind the parameter to the Err type
                                    self.analysis_info.symbols.insert(params[0].clone(), (*err.clone(), false));
                                }
                                return Ok(());
                            }
                        }
                        Type::Custom(n, g) if n == "Result" && g.len() == 2 => {
                            if variant_name == "Ok" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.analysis_info.symbols.insert(params[0].clone(), (g[0].clone(), false));
                                }
                                return Ok(());
                            } else if variant_name == "Err" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.analysis_info.symbols.insert(params[0].clone(), (g[1].clone(), false));
                                }
                                return Ok(());
                            }
                        }
                        Type::Custom(n, _) => { // Infer enum name if possible
                            *enum_name = n.clone();
                        }
                        Type::Any => { // If expression type is Any, bind params to Any
                            for name in params {
                                self.analysis_info.symbols.insert(name.clone(), (Type::Any, false));
                            }
                            return Ok(());
                        }
                        _ => {} // Fall through to other checks if not a known Result pattern
                    }
                }

                // Try to resolve the enum and variant using type information from `types.rs`
                // This requires access to `self.type_info.enums`.
                // Placeholder for actual enum/variant lookup
                if enum_name.is_empty() || enum_name == "any" || self.type_info.is_phantom_type(enum_name) {
                    // If enum name is empty, or 'any', or a phantom type, treat parameters as Any
                    for name in params {
                        self.analysis_info.symbols.insert(name.clone(), (Type::Any, false));
                    }
                    return Ok(());
                }

                // If we couldn't resolve the variant, return an error.
                self.analysis_info.semantic_error(
                    format!("Undeclared variant {}::{}", enum_name, variant_name),
                    span,
                )
            }
            Pattern::Variable(name) => {
                self.analysis_info.symbols.insert(name.clone(), (expr_ty.clone(), false));
                Ok(())
            }
            Pattern::Literal(lit) => {
                // Analyze literal expression using the expression analyzer
                let mut expr_analyzer = ExpressionAnalyzer::new(self.analysis_info, self.type_info);
                expr_analyzer.analyze_expr(lit)?;
                Ok(())
            }
        }
    }
}
able(name) => {
                self.analysis_info.symbols.insert(name.clone(), (expr_ty.clone(), false));
                Ok(())
            }
            Pattern::Literal(lit) => {
                // Analyze literal expression using the expression analyzer
                let mut expr_analyzer = ExpressionAnalyzer::new(self.analysis_info, self.type_info);
                expr_analyzer.analyze_expr(lit)?;
                Ok(())
            }
        }
    }
}
