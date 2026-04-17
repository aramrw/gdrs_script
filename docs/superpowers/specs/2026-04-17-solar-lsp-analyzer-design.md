# Solar LSP & Analyzer Design Specification
**Date:** 2026-04-17
**Status:** Draft

## 1. Overview
This project establishes a minimal, high-performance developer toolchain for the Solar (SR) language. It consists of a Tree-sitter grammar for syntax highlighting and a "hollow" LSP server that provides robust formatting and a foundation for future semantic features.

## 2. Architecture

### 2.1 Tree-sitter Grammar (`crates/tree-sitter-solar`)
A standalone Tree-sitter parser that serves as the "source of truth" for the editor's understanding of Solar code.
- **`grammar.js`**: Defines the full Solar syntax (functions, objects, enums, expressions).
- **`scanner.c`**: Implements an external scanner to handle Solar's indentation-based blocks (`INDENT`, `DEDENT`, `NEWLINE`), using a stack-based approach similar to Python.
- **Highlights**: `queries/highlights.scm` mapping Solar nodes to standard capture groups (e.g., `@function`, `@keyword`).

### 2.2 Analyzer & Formatter (`crates/analyzer`)
A Rust crate providing both a Language Server (LSP) and a standalone CLI formatter.
- **LSP Layer**: Uses `tower-lsp` to handle standard JSON-RPC communication.
    - Supported: `initialize`, `shutdown`, `textDocument/formatting`.
- **Formatter Engine**: Uses the `tree-sitter` Rust bindings.
    - **Logic**: Traverses the parse tree. For every `block` node, it enforces 4-space indentation relative to its parent.
    - **Preservation**: It preserves the literal text of expressions while cleaning up whitespace around operators and block boundaries.
- **CLI**: Supports `analyzer --format <path>` for non-LSP usage.

## 3. Integration Plan

### 3.1 Neovim Setup
- **Filetype**: Detect `.sr` files as `solar`.
- **Parser**: Install the local Tree-sitter parser using `nvim-treesitter`.
- **LSP**: Use a Lua script to spawn the `analyzer` binary and attach it to `solar` buffers.

### 3.2 Formatting Logic
1.  Parse the file into a Tree-sitter CST.
2.  Walk the tree in-order.
3.  For each node:
    - If it's a structural keyword (`fn`, `obj`, `if`), ensure it starts on a new line with current indentation.
    - If it's the start of a `body` or `block`, increment the indentation level.
    - Re-emit the source text with corrected spacing.

## 4. Success Criteria
- Neovim shows correct syntax highlighting for `.sr` files.
- "Format on save" in Neovim corrects indentation to exactly 4 spaces.
- The `analyzer` binary is fast and does not block the editor UI.
