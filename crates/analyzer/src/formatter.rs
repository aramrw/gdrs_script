pub fn format_code(source: &str) -> String {
    let mut formatted = String::new();
    let mut current_indent_level = 0;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            formatted.push('\n');
            continue;
        }

        // 1. Calculate original indentation level (in 4-space units)
        let original_indent = line.chars().take_while(|c| c.is_whitespace()).count() / 4;

        // 2. If we see a dedent in the original file, snap to it
        if original_indent < current_indent_level {
            current_indent_level = original_indent;
        }

        // 3. Apply standard indentation
        let indentation = "    ".repeat(current_indent_level);
        formatted.push_str(&indentation);
        formatted.push_str(trimmed);
        formatted.push('\n');

        // 4. Update indentation level for the next line
        if trimmed.ends_with(':') {
            current_indent_level += 1;
        }
    }

    formatted
}
