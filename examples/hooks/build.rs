fn main() {
    // Trunk moves staging into dist, so cached builds must regenerate this file.
    println!("cargo:rerun-if-changed=dist/.stage/buildGenerated.js");
    println!("cargo:rerun-if-changed=build.rs");
    let function = if std::env::var_os("CARGO_FEATURE_A").is_some() {
        r#"function itDepends() {
    console.log("Feature A is active");
}"#
    } else {
        r#"function itDepends() {
    console.log("Feature A is not active");
}"#
    };

    std::fs::write("dist/.stage/buildGenerated.js", function).expect("must write");
}
