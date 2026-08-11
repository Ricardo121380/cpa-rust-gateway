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

    // The SPA is web/prism since the frontend merge. Its sources are many more
    // files than the old hand-written admin-ui, so the watch list is the build
    // inputs that are not themselves generated: the contract, the build script,
    // the lockfile/config, and the source tree.
    for relative_path in [
        "docs/openapi/management-v1.json",
        "scripts/build-management-spa.sh",
        "web/prism/package-lock.json",
        "web/prism/package.json",
        "web/prism/tsconfig.json",
        "web/prism/vite.config.ts",
        "web/prism/index.html",
        "web/prism/src",
        "web/prism/contracts/management-v1.json",
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

    // Exactly these four, and the SPA's own check.mjs pins the same set from
    // its side (C3): a fifth asset would be served by nothing and must fail the
    // build rather than ship unreachable bytes.
    for relative_path in [
        "web/prism/dist/index.html",
        "web/prism/dist/assets/main.js",
        "web/prism/dist/assets/vendor.js",
        "web/prism/dist/assets/index.css",
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
