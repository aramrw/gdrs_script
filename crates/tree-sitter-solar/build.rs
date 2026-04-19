fn main() {
    let src_dir = std::path::Path::new("src");

    let mut config = cc::Build::new();
    config.include(src_dir);
    config
        .file(src_dir.join("parser.c"))
        .file(src_dir.join("scanner.c"));

    config.compile("tree-sitter-solar");
}
