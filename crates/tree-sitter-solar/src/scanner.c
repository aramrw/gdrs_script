#include <tree_sitter/parser.h>
#include <stdbool.h>

enum TokenType { INDENT, DEDENT, NEWLINE };

void *tree_sitter_solar_external_scanner_create() { return NULL; }
void tree_sitter_solar_external_scanner_destroy(void *payload) {}
unsigned tree_sitter_solar_external_scanner_serialize(void *payload, char *buffer) { return 0; }
void tree_sitter_solar_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {}

bool tree_sitter_solar_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  // Minimal scanner skeleton - will be expanded during implementation
  return false;
}
