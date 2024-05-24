use std::path::PathBuf;

fn main() {
    let dist = PathBuf::from(std::env::var_os("TRUNK_STAGING_DIR").expect("unable eval dist dir"));

    let path = dist.join("generated.js");

    println!("Generating file: {}", path.display());

    let time = time::OffsetDateTime::now_utc();

    std::fs::write(
        path,
        format!(
            r#"function generatedFunction() {{
    console.log("I was generated at {time}");
}}"#
        ),
    )
    .expect("must write file");
}
