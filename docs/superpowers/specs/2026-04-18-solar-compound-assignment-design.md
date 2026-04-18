# Design Specification: Compound Assignment (+=, -=) in Solar

## Overview
Implement compound assignment operators `+=` and `-=` in the Solar language, desugaring them into Rust's `+=` and `-=` operators during transpilation.

## Syntax Changes
Update the parser to recognize `+=` and `-=` as assignment-like statements.

## Parser Changes
1.  **Lexer Updates**:
    *   Add `PlusEq` ("+=") and `MinusEq` ("-=") to `Token` enum in `src/lexer.rs`.
    *   Update `lex` function in `src/lexer.rs` to recognize the `+=` and `-=` sequences.
2.  **Parser Updates**:
    *   Update `stmt_parser` in `src/parser/stmt.rs` to include a rule for `target += value` and `target -= value`.
    *   Map these to a new `StmtKind` variant, e.g., `CompoundAssign(Expr, BinaryOp, Expr)`.

## AST Changes
*   Add `CompoundAssign` variant to `StmtKind` in `src/ast.rs`: `CompoundAssign { target: Expr, op: BinaryOp, value: Expr }`.

## Semantic Analysis
*   Update `src/sema/analysis/statements.rs` to handle `CompoundAssign`. It should ensure the `target` expression is valid (e.g., mutable) and that the operation is valid for the types involved.

## Code Generation (Compiler)
*   Update `src/compiler/statements.rs` to handle `StmtKind::CompoundAssign`.
*   It should desugar `target += value` into:
    ```rust
    {
        let __val = (&#value).as_val();
        #target += __val;
    }
    ```
    This matches the existing `Assign` desugaring.

## Verification
*   Create a test program `test_compound_assign.sr` using `+=` and `-=` and verify it compiles and runs correctly.
