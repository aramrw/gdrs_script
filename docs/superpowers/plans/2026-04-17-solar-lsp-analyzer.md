# Solar LSP & Analyzer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a minimal Solar LSP server with Tree-sitter-based formatting and syntax highlighting.

**Architecture:** A standalone Tree-sitter grammar for Solar and a Rust-based LSP server that leverages it for structural formatting.

**Tech Stack:** Rust, `tower-lsp`, `tree-sitter`, Node.js (for tree-sitter CLI).

---

### Task 1: Tree-sitter Grammar Scaffold

**Files:**
- Create: `crates/tree-sitter-solar/grammar.js`
- Create: `crates/tree-sitter-solar/src/scanner.c`
- Create: `crates/tree-sitter-solar/package.json`

- [ ] **Step 1: Create package.json for Tree-sitter**

```json
{
  "name": "tree-sitter-solar",
  "version": "0.1.0",
  "main": "bindings/node",
  "devDependencies": {
    "tree-sitter-cli": "^0.20.8"
  }
}
```

- [ ] **Step 2: Create grammar.js with basic Solar structure**

```javascript
module.exports = grammar({
  name: 'solar',
  externals: $ => [
    $._indent,
    $._dedent,
    $._newline
  ],
  rules: {
    source_file: $ => repeat($._definition),
    _definition: $ => choice(
      $.function_definition,
      $.object_definition
    ),
    function_definition: $ => seq(
      'fn',
      $.identifier,
      $.parameters,
      optional(seq('->', $.type)),
      ':',
      $.block
    ),
    object_definition: $ => seq(
      'obj',
      $.identifier,
      ':',
      $.block
    ),
    parameters: $ => seq('(', ')'),
    type: $ => $.identifier,
    block: $ => seq(
      $._indent,
      repeat($._statement),
      $._dedent
    ),
    _statement: $ => choice(
      $.expression_statement
    ),
    expression_statement: $ => seq($.identifier, $._newline),
    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/
  }
});
```

- [ ] **Step 2.5: Create scanner.c for indentation handling (Skeleton)**

```c
#include <tree_sitter/parser.h>
#include <vector>

enum TokenType { INDENT, DEDENT, NEWLINE };

void *tree_sitter_solar_external_scanner_create() { return NULL; }
void tree_sitter_solar_external_scanner_destroy(void *payload) {}
unsigned tree_sitter_solar_external_scanner_serialize(void *payload, char *buffer) { return 0; }
void tree_sitter_solar_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {}

bool tree_sitter_solar_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  // Minimal scanner skeleton - will be expanded during implementation
  return false;
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/tree-sitter-solar
git commit -m "feat: scaffold tree-sitter-solar grammar"
```

### Task 2: Analyzer LSP Scaffold

**Files:**
- Create: `crates/analyzer/Cargo.toml`
- Create: `crates/analyzer/src/main.rs`

- [ ] **Step 1: Create analyzer Cargo.toml**

```toml
[package]
name = "analyzer"
version = "0.1.0"
edition = "2021"

[dependencies]
tower-lsp = "0.20"
tokio = { version = "1.0", features = ["full"] }
tree-sitter = "0.20"
# tree-sitter-solar = { path = "../tree-sitter-solar" } # This will need the built library
```

- [ ] **Step 2: Create basic LSP server loop**

```rust
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }

    async fn formatting(&self, _params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        // Formatter logic goes here
        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/analyzer
git commit -m "feat: scaffold analyzer lsp"
```

### Task 3: Formatter Logic Implementation

**Files:**
- Create: `crates/analyzer/src/formatter.rs`
- Modify: `crates/analyzer/src/main.rs`

- [ ] **Step 1: Implement tree-walking formatter logic**

```rust
pub fn format_code(source: &str) -> String {
    // This will eventually use tree-sitter to rebuild the string with correct indents
    source.to_string()
}
```

- [ ] **Step 2: Hook formatter into LSP formatting request**

```rust
// In main.rs, update formatting method
async fn formatting(&self, _params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
    // Placeholder implementation
    Ok(None)
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/analyzer
git commit -m "feat: add formatter skeleton"
```
