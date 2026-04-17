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
