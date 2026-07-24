//! Builds the reviewed management SPA before its exact static bytes are embedded.

use std::{
    env, fmt,
    path::PathBuf,
    process::{Command, exit},
};

fn main() {
    let manifest_directory = match env::var_os("CARGO_MANIFEST_DIR") {
        Some(value) => PathBuf::from(value),
        None => fail("CARGO_MANIFEST_DIR is unavailable"),
    };
    let Some(repository) = manifest_directory.parent().and_then(|path| path.parent()) else {
        fail("could not locate the repository root");
    };

    for relative_path in [
        "docs/openapi/management-v1.json",
        "scripts/build-management-spa.sh",
        "scripts/generate-management-client.mjs",
        "web/admin-ui/package-lock.json",
        "web/admin-ui/package.json",
        "web/admin-ui/tsconfig.json",
        "web/admin-ui/src/generated/management-client.ts",
        "web/admin-ui/src/index.html",
        "web/admin-ui/src/main.ts",
        "web/admin-ui/src/styles.css",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            repository.join(relative_path).display()
        );
    }

    let status = match Command::new("bash")
        .arg(repository.join("scripts/build-management-spa.sh"))
        .current_dir(repository)
        .status()
    {
        Ok(status) => status,
        Err(error) => fail(format_args!(
            "management SPA build could not start: {error}"
        )),
    };
    if !status.success() {
        fail(format_args!(
            "management SPA build failed with status {status}"
        ));
    }

    for relative_path in [
        "web/admin-ui/dist/index.html",
        "web/admin-ui/dist/assets/main.js",
        "web/admin-ui/dist/assets/generated/management-client.js",
        "web/admin-ui/dist/assets/styles.css",
    ] {
        if !repository.join(relative_path).is_file() {
            fail(format_args!(
                "management SPA build omitted required asset {relative_path}"
            ));
        }
    }
}

fn fail(message: impl fmt::Display) -> ! {
    eprintln!("gateway-http-actix build: {message}");
    exit(1)
}
