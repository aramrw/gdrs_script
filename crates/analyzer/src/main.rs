use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use analyzer::formatter;

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

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        
        // In a production LSP, we would maintain a stateful cache of documents.
        // For this "Tiny" version, we'll just read the file from disk or use the params if available.
        // Since tower-lsp doesn't automatically track buffer changes for us without extra work,
        // we'll try to read the file. Note: This won't work for unsaved changes yet.
        
        if let Ok(path) = uri.to_file_path() {
            if let Ok(content) = std::fs::read_to_string(path) {
                let formatted = formatter::format_code(&content);
                
                return Ok(Some(vec![TextEdit {
                    range: Range {
                        start: Position { line: 0, character: 0 },
                        end: Position {
                            line: content.lines().count() as u32 + 1,
                            character: 0,
                        },
                    },
                    new_text: formatted,
                }]));
            }
        }
        
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
