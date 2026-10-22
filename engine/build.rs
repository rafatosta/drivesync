#[path = "oauth_config.rs"]
mod config;
use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE");
    println!("cargo:rerun-if-env-changed=DRIVESYNC_REQUIRE_GOOGLE_OAUTH");
    let explicit = env::var_os("DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE");
    let path = explicit.clone().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("../data/oauth/development.json")
    });
    println!("cargo:rerun-if-changed={}", path.display());
    // Both OAuth environments use the same parser; the build manifest picks
    // the credential file, never the runtime user or a stored refresh token.
    let data = match fs::read_to_string(&path) {
        Ok(data) => {
            let client = config::OAuthConfig::select(None, None, Some(&data), "")
                .unwrap_or_else(|_| panic!("Invalid build OAuth configuration: expected Google Desktop client metadata"));
            serde_json::to_string(&client).unwrap()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && explicit.is_none() => {
            assert!(
                env::var("DRIVESYNC_REQUIRE_GOOGLE_OAUTH").as_deref() != Ok("1"),
                "Release requires the maintainer's Google Desktop OAuth configuration"
            );
            println!("cargo:warning=Google connection unavailable: build has no official Desktop OAuth client");
            String::new()
        }
        Err(_) => panic!("Cannot read build OAuth configuration"),
    };
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("google-oauth.json"),
        data,
    )
    .unwrap();
}
