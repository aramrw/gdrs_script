
// src/sema/analysis/patterns.rs

use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;
use std::collections::HashMap;

// Assuming AnalysisInfo and TypeInfo are accessible or passed as context
// For standalone compilation, these would need to be available.
// In the final structure, they will be passed or accessed via a shared context.

use super::AnalysisInfo;
use crate::sema::types::TypeInfo;
use super::expressions::ExpressionAnalyzer;

pub struct PatternAnalyzer<'a> {
    analysis_info: &'a mut AnalysisInfo,
    type_info: &'a mut TypeInfo,
}

impl<'a> PatternAnalyzer<'a> {
    pub fn new(
        analysis_info: &'a mut AnalysisInfo,
        type_info: &'a mut TypeInfo,
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
                                return Ok(()); // Explicitly return Ok
                            } else if variant_name == "Err" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    // Bind the parameter to the Err type
                                    self.analysis_info.symbols.insert(params[0].clone(), (*err.clone(), false));
                                }
                                return Ok(()); // Explicitly return Ok
                            }
                        }
                        Type::Custom(n, g) if n == "Result" && g.len() == 2 => {
                            if variant_name == "Ok" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.analysis_info.symbols.insert(params[0].clone(), (g[0].clone(), false));
                                }
                                return Ok(()); // Explicitly return Ok
                            } else if variant_name == "Err" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.analysis_info.symbols.insert(params[0].clone(), (g[1].clone(), false));
                                }
                                return Ok(()); // Explicitly return Ok
                            }
                        }
                        Type::Custom(n, _) => { // Infer enum name if possible
                            *enum_name = n.clone();
                            // This case falls through, allowing check against type_info.enums below if needed.
                        }
                        Type::Any => { // If expression type is Any, bind params to Any
                            for name in params {
                                self.analysis_info.symbols.insert(name.clone(), (Type::Any, false));
                            }
                            return Ok(()); // Explicitly return Ok
                        }
                        _ => {} // Fall through to other checks if not a known Result pattern
                    }
                }

                // If enum_name was not empty initially, or was inferred,
                // proceed to check against the type_info.enums.
                // For now, we simplify this by assuming if it's not a special case
                // that returned Ok, it's an error. This part requires proper integration
                // with type_info.enums lookup for a complete implementation.
                // If we reach here and haven't returned Ok, it means we should report an error.
                self.analysis_info.semantic_error(
                    format!("Undeclared variant {}::{}", enum_name, variant_name),
                    span,
                )?;
                Ok(())
            } // <-- Closes Pattern::Variant arm
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
        } // <-- Closes match pattern
    } // <-- Closes fn analyze_pattern
} // <-- Closes impl PatternAnalyzer<'a>
