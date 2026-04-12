# SR Language Phase 1: Core Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate to a C++ backend and implement `var`/`var mut` variable declarations for `i32`.

**Architecture:** Update the Rust compiler to generate `.cpp` files instead of `.c`. Update the AST and parser to support the new variable syntax. The compiler will now invoke `g++`.

**Tech Stack:** Rust, Chumsky (Parsing), C++ (Backend).

---

### Task 1: C++ Backend Migration

**Files:**
- Modify: `src/compiler.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Update compiler to output .cpp and use g++**
Modify `src/compiler.rs` to change `output.c` to `output.cpp` and `gcc` to `g++`.

```rust
// In src/compiler.rs
pub fn compile(func: Function) {
    // ... (logic to generate C++ code)
    fs::write("output.cpp", &c_code).expect("Failed to write C++ code");

    let status = Command::new("g++")
        .args(&["-O3", "output.cpp", "-o", "main_program"])
        .status()
        .expect("Failed to invoke g++. Is it installed?");
    // ...
}
```

- [ ] **Step 2: Update main.rs logging**
Change `[read] ...` to reflect the new process if needed.

- [ ] **Step 3: Verify migration with existing main.sr**
Run `cargo run main.sr` and ensure it still produces a working `main_program` using `g++`.

- [ ] **Step 4: Commit**
```bash
git add src/compiler.rs src/main.rs
git commit -m "feat: migrate backend to C++ (g++)"
```

---

### Task 2: Update AST for Variables and Types

**Files:**
- Modify: `src/ast.rs`

- [ ] **Step 1: Add Type and VarDecl to AST**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I32,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i32),
    Variable(String), // New: access variable
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl {
        name: String,
        is_mutable: bool,
        ty: Type,
        value: Expr,
    },
    Print(Expr),
}
```

- [ ] **Step 2: Commit**
```bash
git add src/ast.rs
git commit -m "feat: add Type and VarDecl to AST"
```

---

### Task 3: Update Parser for Variables

**Files:**
- Modify: `src/parser.rs`

- [ ] **Step 1: Implement Type and Variable parsing**

```rust
// In src/parser.rs
let ty = text::keyword("i32").to(Type::I32).padded();

let var_name = text::ident().padded();

let expr = int.or(var_name.map(|s: &str| Expr::Variable(s.to_string())));

let var_decl = text::keyword("var")
    .padded()
    .then(text::keyword("mut").padded().or_not())
    .then(var_name)
    .then_ignore(just(':'))
    .then(ty)
    .then_ignore(just('='))
    .then(expr)
    .then_ignore(just(';'))
    .map(|((((_, mut_kw), name), ty), value)| Stmt::VarDecl {
        name: name.to_string(),
        is_mutable: mut_kw.is_some(),
        ty,
        value,
    });
```

- [ ] **Step 2: Update main parser to include var_decl**
Ensure `print_stmt.or(var_decl).repeated()` is used in the function body parser.

- [ ] **Step 3: Commit**
```bash
git add src/parser.rs
git commit -m "feat: implement var and var mut parsing"
```

---

### Task 4: Update Compiler for Variables

**Files:**
- Modify: `src/compiler.rs`

- [ ] **Step 1: Update C++ generation for VarDecl**

```rust
// In src/compiler.rs inside the loop
match stmt {
    Stmt::VarDecl { name, is_mutable, ty, value } => {
        let const_kw = if is_mutable { "" } else { "const " };
        let type_str = match ty { Type::I32 => "int32_t" };
        let val_str = match value { 
            Expr::Int(v) => format!("{}LL", v),
            Expr::Variable(n) => n.clone(),
        };
        c_code.push_str(&format!("    {}{} {} = {};\n", const_kw, type_str, name, val_str));
    }
    Stmt::Print(expr) => {
        let val_str = match expr {
            Expr::Int(v) => format!("{}LL", v),
            Expr::Variable(n) => n.clone(),
        };
        c_code.push_str(&format!("    printf(\"%lld\\n\", {});\n", val_str));
    }
}
```

- [ ] **Step 2: Test with a complex main.sr**
Create a test file:
```rust
fn main() {
    var x: i32 = 10;
    var mut y: i32 = 20;
    print(x);
    print(y);
}
```
Run `cargo run main.sr` and verify output.

- [ ] **Step 3: Commit**
```bash
git add src/compiler.rs
git commit -m "feat: generate C++ code for variables"
```
