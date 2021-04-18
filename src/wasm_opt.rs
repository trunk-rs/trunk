use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use async_std::fs;

use crate::common::run_command;
use crate::config::{CargoMetadata, RtcBuild};

pub async fn wasm_opt_build(cfg: Arc<RtcBuild>, manifest: CargoMetadata, level: WasmOptLevel, hashed_name: &str) -> Result<()> {
    // If opt level is off, we skip calling wasm-opt as it wouldn't have any effect.
    if level == WasmOptLevel::Off {
        return Ok(());
    }

    // Ensure our output dir is in place.
    tracing::info!("calling wasm-opt");
    let mode_segment = if cfg.release { "release" } else { "debug" };
    let output = manifest.metadata.target_directory.join("wasm-opt").join(mode_segment);
    fs::create_dir_all(&output).await.context("error creating wasm-opt output dir")?;

    // Build up args for calling wasm-opt.
    let output = output.join(hashed_name);
    let arg_output = format!("--output={}", output.display());
    let arg_opt_level = format!("-O{}", level.as_ref());
    let target_wasm = cfg.staging_dist.join(hashed_name).to_string_lossy().to_string();
    let args = vec![&arg_output, &arg_opt_level, &target_wasm];

    // Invoke wasm-opt.
    run_command("wasm-opt", &args).await?;

    // Copy the generated WASM file to the dist dir.
    tracing::info!("copying generated wasm-opt artifacts");
    fs::copy(output, cfg.staging_dist.join(hashed_name))
        .await
        .context("error copying wasm file to dist dir")?;

    Ok(())
}

/// Different optimization levels that can be configured with `wasm-opt`.
#[derive(PartialEq, Eq)]
pub enum WasmOptLevel {
    /// Default optimization passes.
    Default,
    /// No optimization passes, skipping the wasp-opt step.
    Off,
    /// Run quick & useful optimizations. useful for iteration testing.
    One,
    /// Most optimizations, generally gets most performance.
    Two,
    /// Spend potentially a lot of time optimizing.
    Three,
    /// Also flatten the IR, which can take a lot more time and memory, but is useful on more nested
    /// / complex / less-optimized input.
    Four,
    /// Default optimizations, focus on code size.
    S,
    /// Default optimizations, super-focusing on code size.
    Z,
}

impl FromStr for WasmOptLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "" => Self::Default,
            "0" => Self::Off,
            "1" => Self::One,
            "2" => Self::Two,
            "3" => Self::Three,
            "4" => Self::Four,
            "s" | "S" => Self::S,
            "z" | "Z" => Self::Z,
            _ => bail!("unknown wasm-opt level `{}`", s),
        })
    }
}

impl AsRef<str> for WasmOptLevel {
    fn as_ref(&self) -> &str {
        match self {
            Self::Default => "",
            Self::Off => "0",
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
            Self::Four => "4",
            Self::S => "s",
            Self::Z => "z",
        }
    }
}

impl Default for WasmOptLevel {
    fn default() -> Self {
        // Current default is off until automatic download of wasm-opt is implemented.
        Self::Off
    }
}
