use std::convert::TryFrom;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use async_std::fs;
use colored::*;
use structopt::StructOpt;

/// Build the Rust WASM app and all of its assets.
#[derive(StructOpt)]
#[structopt(name="build")]
pub struct Build {
    /// The name of the app to build.
    app: String, // TODO: make this optional, and point to manifest. Parse ./Cargo.toml by default, err if not present.
    /// The index HTML file.
    #[structopt(parse(from_os_str))]
    target: PathBuf,
    /// The index SASS/SCSS file.
    #[structopt(parse(from_os_str))]
    sass: PathBuf,
    /// Build in release mode.
    #[structopt(long)]
    release: bool,
    /// The output dir for all final assets.
    #[structopt(short, long, default_value="dist", parse(from_os_str))]
    dist: PathBuf,
}

impl Build {
    pub async fn run(&self) -> Result<()> {
        println!("📦 {}", format!("bundling app {}", &self.app).green());
        let data = BuildData::try_from(self)?;

        // Start the cargo build in the background.
        let mut args = vec!["build", "--target=wasm32-unknown-unknown"];
        if data.release {
            args.push("--release");
        }
        let build_result = std::thread::spawn(move || {
            println!("📦 {}", "starting cargo build".green());
            std::process::Command::new("cargo")
                .args(args.as_slice())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?
                .wait_with_output()
        });

        // Parse the target HTML document.
        //
        // TODO:NOTE: pretty much all of the HTML parsing/manipulation libs currently out there SUCK!
        // We'll have to tackle this at some point. The best new one which appears to have promise:
        // https://github.com/mathiversen/html-parser
        // Probably what we can do is parse using that lib, and then build the output via https://github.com/http-rs/html-index
        // ----
        // let file = fs::read_to_string(&self.target).await?;
        // println!("{}{}", "HTML file read:\n".green(), file.white());

        // Ensure output directories are in place.
        fs::create_dir_all(&data.dist).await?;
        fs::create_dir_all(&data.bindgen_out).await?;
        let dist_path = data.dist.display();
        let bindgen_path = data.bindgen_out.display();

        // Start the sass/scss pipeline.
        //
        // TODO:NOTE: long-term this will depend on the config in the HTML file.
        let output = match grass::from_path(data.sass.to_string_lossy().as_ref(), &grass::Options::default()) {
            Ok(output) => output,
            Err(err) => {
                println!("{}{}", "error compiling sass\n".red(), err.to_string().red());
                return Ok(());
            }
        };

        // Compute SHA1 digest over content.
        let sass_file = format!("index.{:x}.css", seahash::hash(output.as_bytes()));
        fs::write(format!("{}/{}", &dist_path, sass_file), output).await?;
        println!("📦 {}", format!("compiled SASS/SCSS written to {}", &dist_path).green());

        // Handle build results.
        match build_result.join() {
            Ok(Ok(output)) => {
                // If the bulid failed, print the output, else continue.
                if !output.status.success() {
                    println!("{}", String::from_utf8_lossy(&output.stderr).red());
                    return Err(anyhow!("cargo build failed"));
                }
                println!("📦 {}", "cargo build completed successfully".green());
            }
            Ok(Err(err)) => {
                // This represents some error with the spawned build process.
                println!("{}{}", "error while building app".red(), err.to_string().red());
                return Err(anyhow!("failed to build app"));
            }
            Err(err) => {
                // This represents some error with the thread itself.
                println!("{}{:?}", "error while attempting build app".red(), err);
                return Err(anyhow!("error with spawned build process"));
            },
        }

        // Hash the built wasm app, then use that as the out-name param.
        let wasm_bytes = fs::read(&data.app_target_wasm).await?;
        let hashed_name = format!("index-{:x}", seahash::hash(&wasm_bytes)); // NOTE: I want to use a dot here, but wasm-bindgen is cutting off
                                                                             // everything after first period. So just `-` for now.

        // Spawn the wasm-bindgen call to perform that last leg of application setup.
        let arg_out_path = format!("--out-dir={}", bindgen_path);
        let arg_out_name = format!("--out-name={}", hashed_name);
        let target_wasm = data.app_target_wasm.to_string_lossy().to_string();
        let bindgen_result = std::thread::spawn(move || {
            let args = vec!["--target=web", &arg_out_path, &arg_out_name, "--no-typescript", &target_wasm];
            println!("📦 {}", "starting wasm-bindgen build".green());
            std::process::Command::new("wasm-bindgen")
                .args(args.as_slice())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?
                .wait_with_output()
        });
        match bindgen_result.join() {
            Ok(Ok(output)) => {
                // If the bulid failed, print the output, else continue.
                if !output.status.success() {
                    println!("{}", String::from_utf8_lossy(&output.stderr).red());
                    return Err(anyhow!("wasm-bindgen build failed"));
                }
                println!("📦 {}", "wasm-bindgen build completed successfully".green());
            }
            Ok(Err(err)) => {
                // This represents some error with the spawned build process.
                println!("{}{}", "error during wasm-bindgen routine".red(), err.to_string().red());
                return Err(anyhow!("wasm-bindgen error"));
            }
            Err(err) => {
                // This represents some error with the thread itself.
                println!("{}{:?}", "error while invoking wasm-bindgen".red(), err);
                return Err(anyhow!("error with spawned wasm-bindgen process"));
            },
        }

        // Copy the generated WASM & JS loader to the dist dir, and generate the needed body
        // for the output HTML.
        let hashed_js_name = format!("{}.js", hashed_name);
        let hashed_wasm_name = format!("{}_bg.wasm", hashed_name);
        let js_loader_path = data.bindgen_out.join(&hashed_js_name);
        let js_loader_path_dist = data.dist.join(&hashed_js_name);
        let wasm_path = data.bindgen_out.join(&hashed_wasm_name);
        let wasm_path_dist = data.dist.join(&hashed_wasm_name);
        fs::copy(js_loader_path, js_loader_path_dist).await?;
        fs::copy(wasm_path, wasm_path_dist).await?;

        // Assemble a new output index.html file.
        let html = html_index::Builder::new()
            .raw_body(&build_html_body(&hashed_wasm_name, &hashed_js_name))
            // .script("/index.js")
            .blocking_style(&format!("/{}", sass_file))
            .title("trunk | pack your things, we’re going on a trip!")
            .build();
        fs::write(format!("{}/index.html", &dist_path), html).await?;
        println!("📦 {}", format!("compiled HTML written to {}", &dist_path).green());

        Ok(())
    }
}

/// Fully validated data related to a build.
struct BuildData {
    /// The name of the app to build.
    pub app: String,
    /// The index HTML file.
    pub target: PathBuf,
    /// The index SASS/SCSS file.
    pub sass: PathBuf,

    /// Build in release mode.
    pub release: bool,
    /// The output dir for all final assets.
    pub dist: PathBuf,
    /// The output dir of the wasm-bindgen execution.
    pub bindgen_out: PathBuf,

    /// The path to the app's output WASM.
    pub app_target_wasm: PathBuf,
}

impl TryFrom<&'_ Build> for BuildData {
    type Error = anyhow::Error;
    fn try_from(src: &'_ Build) -> Result<Self> {
        // TODO: update these to be based on Cargo manifest's target dir.
        let app_target_wasm = std::env::current_dir()?
            .join("target")
            .join("wasm32-unknown-unknown")
            .join(if src.release { "release" } else { "debug" })
            .join(format!("{}.wasm", &src.app));
        let bindgen_out = std::env::current_dir()?
            .join("target")
            .join("wasm-bindgen");

        Ok(Self{
            app: src.app.clone(),
            target: src.target.canonicalize().map_err(|err| anyhow!("error with target html file: {}", err))?,
            sass: src.sass.canonicalize().map_err(|err| anyhow!("error with target sass file: {}", err))?,
            release: src.release,
            dist: src.dist.clone(),
            bindgen_out,
            app_target_wasm,
        })
    }
}

fn build_html_body(wasm_file: &str, js_loader_file: &str) -> String {
    format!(
        r#"<body><script type="module">import init from '/{js}';init('/{wasm}');</script></body>"#,
        js=js_loader_file, wasm=wasm_file,
    )
}
