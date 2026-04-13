# Mini-Rust (Solar) Architecture & Vision

## 1. Core Identity & Use Case
The language (currently codenamed Solar, transitioning to a "Mini-Rust" concept) is a **Hot-Reloadable Systems Scripting Language**. It is designed to be embedded within or interact seamlessly with large, heavy Rust host applications (e.g., game engines, UI frameworks like GPUI).

### The Killer Feature: Sub-Second Native Iteration
Instead of waiting for `rustc` to compile a massive application on every logic tweak, this language transpiles to Rust and compiles into a small dynamic library (`.dylib` / `.so` / `.dll`). The host application hot-reloads this library instantly upon saving.
- **Result:** Python/GDScript-like iteration speed for business logic, UI layouts, and event handlers, while executing as 100% native, optimized Rust code.

## 2. Syntax & Ergonomics
- **Mojo/Python-like syntax:** Indentation-based, minimal boilerplate, no braces or semicolons.
- **Explicit but Ergonomic:** Retains Rust's clarity on what is happening under the hood (e.g., explicit references `&obj` to indicate reading without taking ownership) without the visual noise.

## 3. Memory & Lifetime Model
The hardest part of Rust (lifetimes) is hidden or simplified, without resorting to a Garbage Collector.

### 3.1 The Reference Rule (Simple Borrow Checking)
- **Explicit References:** The language supports `&` and `&mut` to pass references to functions, making performance characteristics obvious.
- **No Lifetime Syntax:** Users are **not allowed** to write explicit lifetime annotations (like `'a`).
- **Standard Elision:** The transpiler relies on standard Rust lifetime elision rules for 95% of function calls.
- **The Escape Hatch Error:** If a user attempts to store a reference in a struct or pass it to a background thread (actions that require explicit lifetimes in Rust), the compiler will throw a friendly error: *"Lifetime escapes local scope. Use `Arc` or `Box` instead."*

### 3.2 Memory Management
- **No GC:** The language adheres to Rust's RAII (Resource Acquisition Is Initialization) memory model.
- **First-Class Smart Pointers:** For data that must escape local scopes or be shared across threads, `Arc` and `Box` are seamlessly integrated and encouraged as the standard solution.

## 4. Rust Interop (The "Glue" Layer)
- **Seamless Imports:** Can import and use standard Rust crates (e.g., `serde`, `tokio`, `rand`) directly.
- **Host Communication:** Capable of calling complex, heavy functions defined in the host Rust application.
- **Async/Await:** Natively understands async paradigms, allowing it to poll futures or spawn background tasks using the host's async runtime (e.g., smol, tokio).

## 5. Scope & Future Implementation
This is a highly ambitious undertaking that effectively requires writing a simplified Rust compiler frontend. 
**Next Steps:**
1. Refactor the existing parser/lexer to support `&` and `&mut` tokens.
2. Update the semantic analyzer to enforce the "No Explicit Lifetimes" rule and provide the escape hatch errors.
3. Build the hot-reloading dynamic library infrastructure (`.dylib` compilation and host injection).
