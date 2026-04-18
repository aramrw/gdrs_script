# Compound Assignment (+=, -=) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `+=` and `-=` operators in the Solar language, transpiled to Rust's compound assignment operators.

**Architecture:** Extend the lexer and parser to recognize the new tokens, add a `CompoundAssign` variant to the AST's `StmtKind`, and update the compiler to handle the transpilation.

**Tech Stack:** Rust, Chumsky (parser), Proc-Macro2/Quote (transpilation).

---

### Task 1: Lexer Update

**Files:**
- Modify: `src/lexer.rs`

- [ ] **Step 1: Add tokens**

Add `PlusEq` and `MinusEq` variants to `Token` enum in `src/lexer.rs`.

- [ ] **Step 2: Update `to_string`**

Update `Token::to_string` in `src/lexer.rs` to include `+=` and `-=`.

- [ ] **Step 3: Update `lex` function**

Update `lex` in `src/lexer.rs` to recognize `+=` and `-=`.

- [ ] **Step 4: Commit**

```bash
git add src/lexer.rs
git commit -m "feat: lexer support for += and -="
```

### Task 2: AST Update

**Files:**
- Modify: `src/ast.rs`

- [ ] **Step 1: Add `CompoundAssign` variant**

Add `CompoundAssign { target: Expr, op: BinaryOp, value: Expr }` to `StmtKind` in `src/ast.rs`.

- [ ] **Step 2: Commit**

```bash
git add src/ast.rs
git commit -m "feat: ast support for compound assignment"
```

### Task 3: Parser Update

**Files:**
- Modify: `src/parser/stmt.rs`

- [ ] **Step 1: Add compound assignment rule**

Add `compound_assign` rule to `stmt_parser` in `src/parser/stmt.rs`.

- [ ] **Step 2: Update choice**

Add `compound_assign` to the `choice` in `stmt_parser`.

- [ ] **Step 3: Commit**

```bash
git add src/parser/stmt.rs
git commit -m "feat: parser support for += and -="
```

### Task 4: Semantic Analysis Update

**Files:**
- Modify: `src/sema/analysis/statements.rs`

- [ ] **Step 1: Handle `CompoundAssign`**

Add logic to `analyze_stmt` in `src/sema/analysis/statements.rs` to check for `CompoundAssign`.

- [ ] **Step 2: Commit**

```bash
git add src/sema/analysis/statements.rs
git commit -m "feat: sema support for += and -="
```

### Task 5: Compiler/Codegen Update

**Files:**
- Modify: `src/compiler/statements.rs`

- [ ] **Step 1: Add `CompoundAssign` compilation**

Add `StmtKind::CompoundAssign` handling to `compile_stmt` in `src/compiler/statements.rs`.

- [ ] **Step 2: Commit**

```bash
git add src/compiler/statements.rs
git commit -m "feat: compiler support for += and -="
```

### Task 6: Testing

**Files:**
- Create: `examples/test_compound_assign.sr`

- [ ] **Step 1: Write the test program**

Create `examples/test_compound_assign.sr` that uses `+=` and `-=`.

- [ ] **Step 2: Run and verify**

```bash
cargo run -- examples/test_compound_assign.sr
./sr_test_compound_assign
```

- [ ] **Step 3: Commit**

```bash
git add examples/test_compound_assign.sr
git commit -m "test: add test_compound_assign.sr"
```
