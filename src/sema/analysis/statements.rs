
// src/sema/statements.rs

use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;
use std::collections::HashMap; // Required for context if called standalone

use super::AnalysisInfo;
use crate::sema::types::TypeInfo;
use super::expressions::ExpressionAnalyzer;
use super::patterns::PatternAnalyzer;

pub struct StatementAnalyzer<'a> {
    analysis_info: &'a mut AnalysisInfo,
    type_info: &'a mut TypeInfo,
}

impl<'a> StatementAnalyzer<'a> {
    pub fn new(
        analysis_info: &'a mut AnalysisInfo,
        type_info: &'a mut TypeInfo,
    ) -> Self {
        Self { analysis_info, type_info }
    }

    pub fn analyze_stmt(&mut self, stmt: &mut Stmt) -> Result<(), CompilerError> {
        let span = stmt.span;
        match &mut stmt.kind {
            StmtKind::VarDecl {
                name,
                is_mutable,
                ty,
                value,
            } => {
                self.analysis_info
                    .var_declarations
                    .insert(name.clone(), (self.analysis_info.scope_depth, *is_mutable));
                let val_ty = self.analyze_expr(value)?; // Use expression analyzer
                let expected_ty = ty.as_ref().map(|t| self.type_info.resolve_type(t, &self.analysis_info.current_prefix));
                if let Some(et) = expected_ty {
                    if !self.analysis_info.types_equal(&et, &val_ty) {
                        self.analysis_info.semantic_error(
                            format!(
                                "Type mismatch in declaration: expected {:?}, found {:?}",
                                et, val_ty
                            ),
                            span,
                        )?;
                    }
                }
                self.analysis_info.symbols.insert(name.clone(), (val_ty, false));
                Ok(())
            }
            StmtKind::Assign { target, value } => {
                let t_ty = self.analyze_expr(target)?;
                let v_ty = self.analyze_expr(value)?;
                if !self.analysis_info.types_equal(&t_ty, &v_ty) {
                    self.analysis_info.semantic_error(
                        format!("Type mismatch in assignment: {:?} and {:?}", t_ty, v_ty),
                        span,
                    )?;
                }
                Ok(())
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.analysis_info.types_equal(&cond_ty, &Type::Bool) {
                    self.analysis_info.semantic_error(
                        format!("If condition must be bool, found {:?}", cond_ty),
                        span,
                    )?;
                }
                self.analyze_stmt(then_branch)?;
                if let Some(eb) = else_branch {
                    self.analyze_stmt(eb)?;
                }
                Ok(())
            }
            StmtKind::While { condition, body } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.analysis_info.types_equal(&cond_ty, &Type::Bool) {
                    self.analysis_info.semantic_error(
                        format!("While condition must be bool, found {:?}", cond_ty),
                        span,
                    )?;
                }
                self.analyze_stmt(body)?;
                Ok(())
            }
            StmtKind::For { var_name, iterator, body } => {
                let _iter_ty = self.analyze_expr(iterator)?;
                
                let old_symbols = self.analysis_info.symbols.clone();
                self.analysis_info.scope_depth += 1;
                
                // For now, treat the loop variable as Type::Any
                self.analysis_info.symbols.insert(var_name.clone(), (Type::Any, false));
                
                self.analyze_stmt(body)?;
                
                self.analysis_info.scope_depth -= 1;
                self.analysis_info.symbols = old_symbols;
                Ok(())
            }
            StmtKind::Loop { body } => {
                self.analyze_stmt(body)?;
                Ok(())
            }
            StmtKind::Block(stmts) | StmtKind::UnsafeBlock(stmts) => {
                self.analysis_info.scope_depth += 1;
                for s in stmts {
                    self.analyze_stmt(s)?;
                }
                self.analysis_info.scope_depth -= 1;
                Ok(())
            }
            StmtKind::ExprStmt(expr) => {
                self.analyze_expr(expr)?;
                Ok(())
            }
            StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    self.analyze_expr(e)?;
                    self.analysis_info.detect_escape(e);
                }
                Ok(())
            }
            StmtKind::Match { expr, arms } => {
                let expr_ty = self.analyze_expr(expr)?;
                for arm in arms {
                    let old_symbols = self.analysis_info.symbols.clone();
                    self.analysis_info.scope_depth += 1;
                    self.analyze_pattern(&mut arm.pattern, &expr_ty, span)?;
                    self.analyze_stmt(&mut arm.body)?;
                    self.analysis_info.scope_depth -= 1;
                    self.analysis_info.symbols = old_symbols;
                }
                Ok(())
            }

            _ => Ok(()), // Ignore other statement types for now
        }
    }

    pub fn refine_stmt(&mut self, stmt: &mut Stmt) -> Result<(), CompilerError> {
        match &mut stmt.kind {
            StmtKind::VarDecl { name: _, value, .. } => {
                self.refine_expr(value)?;
            }
            StmtKind::Assign { target, value } => {
                self.refine_expr(target)?;
                self.refine_expr(value)?;
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.refine_expr(condition)?;
                self.refine_stmt(then_branch)?;
                if let Some(eb) = else_branch {
                    self.refine_stmt(eb)?;
                }
            }
            StmtKind::While { condition, body } => {
                self.refine_expr(condition)?;
                self.refine_stmt(body)?;
            }
            StmtKind::For { iterator, body, .. } => {
                self.refine_expr(iterator)?;
                self.refine_stmt(body)?;
            }
            StmtKind::Loop { body } => {
                self.refine_stmt(body)?;
            }
            StmtKind::Block(stmts) | StmtKind::UnsafeBlock(stmts) => {
                for s in stmts {
                    self.refine_stmt(s)?;
                }
            }
            StmtKind::ExprStmt(expr) => {
                self.refine_expr(expr)?;
            }
            StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    self.refine_expr(e)?;
                }
            }
            _ => {} // Do nothing for other statement types
        }
        Ok(())
    }

    // Helper methods that require access to ExpressionAnalyzer and TypeInfo
    fn analyze_expr(&mut self, expr: &mut Expr) -> Result<Type, CompilerError> {
        let mut expr_analyzer = ExpressionAnalyzer::new(self.analysis_info, self.type_info);
        expr_analyzer.analyze_expr(expr)
    }

    fn refine_expr(&mut self, expr: &mut Expr) -> Result<(), CompilerError> {
        let mut expr_analyzer = ExpressionAnalyzer::new(self.analysis_info, self.type_info);
        expr_analyzer.refine_expr(expr)
    }

    fn analyze_pattern(&mut self, pattern: &mut Pattern, expr_ty: &Type, span: Span) -> Result<(), CompilerError> {
        let mut pattern_analyzer = PatternAnalyzer::new(self.analysis_info, self.type_info);
        pattern_analyzer.analyze_pattern(pattern, expr_ty, span)
    }
}
