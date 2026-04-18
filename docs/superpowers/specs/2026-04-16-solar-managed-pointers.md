# Spec: Solar Managed Pointer System (The "Headache-Free" Borrowing Model)

## 1. Overview
Solar (SR) is designed to provide "syntactic sugar on top of Rust," offering a high-level, ergonomic developer experience without sacrificing Rust's performance and system-level control. This spec defines the **Managed Pointer System**, which introduces tiered ownership and automatic memory promotion to simplify borrowing.

## 2. The Pointer Tiers
Solar classifies references into three primary tiers based on their scope and thread-safety requirements.

| Tier | Solar Syntax | Rust Backend | Character |
| :--- | :--- | :--- | :--- |
| **Borrow** | `&T`, `&mut T` | `&T`, `&mut T` | Zero-cost, temporary access. |
| **Local Managed** | `*T` | `Rc<RefCell<T>>` | Shared ownership within a single thread. |
| **Global Managed**| `**T` | `Arc<RwLock<T>>` | Thread-safe shared ownership. |

### 2.1. Unsafe & Explicit Pointers
For advanced users who require raw control, Solar maintains a direct mapping to Rust's unsafe pointers and heap allocations.

*   `*const T`: Raw constant pointer.
*   `*mut T`: Raw mutable pointer.
*   `Box<T>`: Unique heap allocation (transparently uses Rust's `Box`).

## 3. Explicit Memory Allocation
Solar provides explicit keywords for manual heap allocation. These forms are **never** subject to automatic promotion.

*   `*box {expr}`: Allocates the result of the expression on the heap in a `Box`.
*   `*mut {expr}`: Creates a raw mutable pointer (unsafe).
*   `*const {expr}`: Creates a raw constant pointer (unsafe).

## 4. The Promotion Engine (Escape Analysis)
To eliminate the "lifetime headache," Solar automatically promotes plain local variables to managed types if the compiler detects they "escape" their scope.

### 4.1. Promotion Triggers
A variable `p` is promoted if:
1.  A reference to `p` (`&p`) is returned from the function.
2.  A reference to `p` is stored in a struct field that outlives the current scope.
3.  A reference to `p` is captured by a long-lived closure.

### 4.2. Promotion Logic
*   **Time-Travel Tagging:** When promotion is triggered, the compiler tags the *original declaration* of the variable.
*   **Local Escape:** Variables are promoted to `*T` (Local Managed) by default.
*   **Async/Thread Escape:** If the escape occurs across an `async` or thread boundary, the variable is promoted to `**T` (Global Managed).

### 4.3. Exclusions
Explicitly allocated variables (`*box`, `*mut`, `*const`) are exempt from automatic promotion.

## 5. Transparent Borrowing (The "Glue")
The transpiler handles the "ceremony" of managed types at function boundaries.

### 5.1. Down-casting (Passing Managed to Borrow)
If a function signature expects `&T`, but is provided a `*T` or `**T`, Solar automatically inserts the borrowing logic.

*   `fn f(p: &T)` called with `p: *T` → `f(&*p.borrow())`
*   `fn f(p: &mut T)` called with `p: *T` → `f(&mut *p.borrow_mut())` (May panic at runtime if already borrowed).
*   `fn f(p: &T)` called with `p: **T` → `f(&*p.read().unwrap())`

### 5.2. Field Access
Field access on managed pointers is transparent. `p.x` is automatically expanded to the correct borrow/lock call depending on whether it is a read or write access.

## 6. Implementation Strategy

### 6.1. AST Updates
*   Add `Type::Managed(Box<Type>)` and `Type::ThreadSafe(Box<Type>)`.
*   Add `ExprKind::PromoteToManaged` for internal use.

### 6.2. Semantic Analysis Pass
*   Implement a new pass for **Escape Analysis**.
*   Maintain a mapping of `Variable -> ScopeLevel`.
*   Identify "escaping" references and update variable types in the AST.

### 6.3. Transpiler (Compiler) Updates
*   Update `compile_type` to map `*T` to `Rc<RefCell<T>>` and `**T` to `Arc<RwLock<T>>`.
*   Update `compile_expr` to insert `.borrow()` / `.read()` at call sites and member access.

## 7. Diagnostics
Solar will emit informational warnings when promotion occurs:
`[Info] Promoting 'p' to managed (*) due to usage at line 42.`
A compiler flag `--no-implicit-promotion` will be available for users who want strict Rust-like behavior.
