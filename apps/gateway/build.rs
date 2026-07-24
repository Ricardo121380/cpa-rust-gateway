//! Embeds a release identity supplied by the P12 build workflow without reading configuration.

use std::env;

const BUILD_FIELDS: [(&str, &str); 3] = [
    ("GATEWAY_RELEASE_REVISION", "development"),
    ("GATEWAY_RELEASE_RUST_VERSION", "development"),
    ("GATEWAY_RELEASE_TARGET", "development"),
];

fn build_field(name: &str, fallback: &str) -> Result<String, String> {
    let value = env::var(name).unwrap_or_else(|_| fallback.to_owned());
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(format!("{name} must be a non-empty printable ASCII value"));
    }
    Ok(value)
}

fn main() -> Result<(), String> {
    for (name, fallback) in BUILD_FIELDS {
        println!("cargo:rerun-if-env-changed={name}");
        println!("cargo:rustc-env={name}={}", build_field(name, fallback)?);
    }
    Ok(())
}
