# SR File I/O Primitives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement fast, hardcoded File I/O primitives in the SR language that map directly to Rust's `std::fs::File`.

**Architecture:** 
- Add a built-in `file` type to the AST and Lexer.
- Map SR `file` type to Rust `std::fs::File` in the compiler.
- Implement low-level "magic functions" (`file_open`, `file_read`, `file_write`) as compiler-provided built-ins.
- These primitives will allow SR users to build higher-level I/O abstractions in pure SR.

**Tech Stack:** Rust, chumsky, quote, proc_macro2.

---

### Task 1: Lexer and AST Extensions

**Files:**
- Modify: `src/lexer.rs`
- Modify: `src/ast.rs`

- [ ] **Step 1: Add `File` to `Token` enum in `src/lexer.rs`**

```rust
pub enum Token {
    // ... existing ...
    File, // Add this
}
```

- [ ] **Step 2: Add `file` keyword to identifier mapping in `src/lexer.rs`**

```rust
"file" => tokens.push(Token::File),
```

- [ ] **Step 3: Add `File` to `Type` enum in `src/ast.rs`**

```rust
pub enum Type {
    // ... existing ...
    File,
}
```

- [ ] **Step 4: Commit**

```bash
git add src/lexer.rs src/ast.rs
git commit -m "ast: add file primitive type"
```

---

### Task 2: Parser Integration

**Files:**
- Modify: `src/parser/mod.rs`

- [ ] **Step 1: Update `type_parser` to recognize `file` as a type**

```rust
fn type_parser<'a>() -> ... {
    recursive(|ty| {
        let base = choice((
            // ... existing ...
            just(Token::File).to(Type::I32), // TEMPORARY mapping to I32 to verify lexer, will change to Type::File in Step 2
        ));
        // ...
    })
}
```

- [ ] **Step 2: Correct `type_parser` to return `Type::File`**

```rust
fn type_parser<'a>() -> ... {
    recursive(|ty| {
        let base = choice((
            // ... existing ...
            just(Token::I32).to(Type::I32),
            just(Token::File).to(Type::File), // Proper mapping
            // ...
        ));
        // ...
    })
}
```

- [ ] **Step 3: Commit**

```bash
git add src/parser/mod.rs
git commit -m "parser: support file type in declarations"
```

---

### Task 3: Semantic Analysis for File Built-ins

**Files:**
- Modify: `src/sema/mod.rs`

- [ ] **Step 1: Register built-in file functions in `analyze()`**

```rust
// Inside analyze() before the main analysis loop
self.functions.insert("file_open".to_string(), (vec![Type::Str], Some(Type::File)));
self.functions.insert("file_read".to_string(), (vec![Type::File], Some(Type::Str)));
self.functions.insert("file_write".to_string(), (vec![Type::File, Type::Str], None));
```

- [ ] **Step 2: Update `analyze_expr` to handle `Type::File`**

Ensure `Type::File` is handled in equality checks or relevant expression contexts if needed.

- [ ] **Step 3: Commit**

```bash
git add src/sema/mod.rs
git commit -m "sema: register file I/O built-in functions"
```

---

### Task 4: Compiler Support for File I/O

**Files:**
- Modify: `src/compiler/mod.rs`

- [ ] **Step 1: Map `Type::File` to `std::fs::File` in `compile_type()`**

```rust
match ty {
    // ... existing ...
    Type::File => quote!(std::fs::File),
}
```

- [ ] **Step 2: Implement built-in file functions in `compile_expr()`**

```rust
Expr::Call(name, args) => {
    if name == "file_open" {
        let path = compile_expr(&args[0], target_obj);
        return quote! { std::fs::File::open(#path).expect("Failed to open file") };
    }
    if name == "file_read" {
        let f = compile_expr(&args[0], target_obj);
        return quote! { {
            use std::io::Read;
            let mut s = String::new();
            let mut f_handle = #f;
            f_handle.read_to_string(&mut s).expect("Failed to read file");
            s
        } };
    }
    if name == "file_write" {
        let f = compile_expr(&args[0], target_obj);
        let content = compile_expr(&args[1], target_obj);
        return quote! { {
            use std::io::Write;
            let mut f_handle = #f;
            f_handle.write_all(#content.as_bytes()).expect("Failed to write file");
        } };
    }
    // ... existing ...
}
```

- [ ] **Step 3: Commit**

```bash
git add src/compiler/mod.rs
git commit -m "compiler: implement file I/O primitives"
```

---

### Task 5: Verification

**Files:**
- Create: `file_test.sr`

- [ ] **Step 1: Create a test program in SR**

```python
fn main():
    // Write a file
    var f_write = file_open("test.txt") // Wait, open needs a mode. Let's simplify to create for now.
    // Actually, let's use a more primitive name: file_create
```

- [ ] **Step 2: Refine built-ins to be more practical**

Rename `file_open` to `file_create` for writing if needed, or add `file_create`.
Let's stick to `file_create` and `file_open`.

- [ ] **Step 3: Update Plan Task 3 & 4 with `file_create`**

- [ ] **Step 4: Run the test program and verify `test.txt` content**

Run: `cargo run -- file_test.sr && cat test.txt`
Expected: Content matches what was written.

- [ ] **Step 5: Commit final test**

```bash
git add file_test.sr
git commit -m "test: verify file I/O primitives"
```
