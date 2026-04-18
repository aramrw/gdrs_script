#!/usr/bin/env nu

def main [file: string] {
    # Extract the filename without .sr and format the binary name
    let binary_name = $"sr_($file | path parse | get stem)"

    # 1. Run the compiler
    cargo run -- $file

    # 2. Run the generated binary
    # We use ^ to ensure we are calling an external command/file
    ^$"./($binary_name)"

    # 3. Clean up
    rm $binary_name
}
