use lang::run_compiler;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_compile_examples() {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let entries = fs::read_dir(examples_dir).expect("Failed to read examples directory");

    let mut failed = Vec::new();

    for entry in entries {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("sr") {
            println!("Testing compilation of: {:?}", path);
            // Some examples might require special features or environment, 
            // but for now let's try to compile all.
            // We'll skip ones that are known to fail if necessary.
            
            // For now, just try to compile.
            if let Err(e) = run_compiler(path.to_str().unwrap()) {
                failed.push((path, e));
            }
        }
    }

    if !failed.is_empty() {
        for (path, err) in &failed {
            eprintln!("Failed to compile {:?}:\n{}", path, err);
        }
        panic!("{} examples failed to compile", failed.len());
    }
}
