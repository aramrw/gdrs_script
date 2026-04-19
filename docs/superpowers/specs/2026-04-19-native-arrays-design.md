# 1-to-1 Native Array Implementation Design

## 1. Compiler Changes
*   **Type Representation**: The `Type::Array(Box<Type>, usize)` will be treated as a primitive type in the compiler.
*   **Transpilation**:
    *   `[T; N]` -> `[T; N]` (native Rust array).
    *   Initialization `[val; N]` -> `[val; N]`.
    *   Access `arr[i]` -> `arr[i]`.
*   **Removal of Pointer Logic**: All code paths in `compiler/expressions.rs` and `types.rs` that convert arrays to `*mut T` or rely on `RawVector` will be stripped.

## 2. Transpilation Strategy
*   No abstraction, no wrappers, no custom bounds checking within Solar.
*   We rely on Rust's inherent array safety. If an out-of-bounds error occurs at runtime, it will panic via the generated Rust code.
*   Compiler error messages will bubble up from the underlying `rustc` invocation, associated with the original Solar file/line via the existing mapping logic.

## 3. Standard Library (`std/vec/mod.sr`) Refactoring
*   The `std::vec::Vec` module will be refactored to simply wrap `std::vec::Vec` from Rust's standard library.
*   `RawVector` and all its associated pointer-based implementation blocks will be removed from `std/vec/mod.sr`.
*   `Vec<T>` methods will directly call the corresponding methods on the internal Rust `Vec<T>`.

---
**Status:** Approved.
**Location:** `docs/superpowers/specs/2026-04-19-native-arrays-design.md`
