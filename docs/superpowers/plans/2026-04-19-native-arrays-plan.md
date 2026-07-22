# Native Array Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 1-to-1 native array transpilation and refactor `std::vec::Vec`.

**Architecture:** Transpile `[T; N]` to `[T; N]` and remove pointer-based `RawVector`.

**Tech Stack:** Rust, Solar Compiler.

---

### Task 1: Remove Pointer Logic from Compiler
- [ ] Modify `src/compiler/expressions.rs` to remove `RawPtr` logic for arrays.
- [ ] Modify `src/compiler/types.rs` to remove pointer conversion for `Type::Array`.
- [ ] Verify `compiler/mod.rs` no longer treats arrays as pointers.

### Task 2: Refactor `std/vec/mod.sr`
- [ ] Remove `RawVector` definition and implementation.
- [ ] Update `Vec<T>` to wrap `std::vec::Vec` directly.
- [ ] Ensure `std/vec/mod.sr` methods interface correctly with native `Vec<T>`.

### Task 3: Verify & Run
- [ ] Compile a simple array-based test.
- [ ] Ensure index access panics correctly at runtime when out of bounds.
- [ ] Commit all changes.
