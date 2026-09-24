//! Compiles the trimmed mbedTLS (and the small C wrapper around it) for the
//! `tls` feature. Sources come from vendor/mbedtls (scripts/fetch-mbedtls.sh)
//! or from the directory in MBEDTLS_DIR.

fn main() {
    println!("cargo:rerun-if-changed=csrc");
    println!("cargo:rerun-if-env-changed=MBEDTLS_DIR");
    #[cfg(feature = "tls")]
    tls::build();
}

#[cfg(feature = "tls")]
mod tls {
    use std::path::{Path, PathBuf};

    pub fn build() {
        let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let root = std::env::var_os("MBEDTLS_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("vendor/mbedtls"));
        if !root.join("library/ssl_tls.c").is_file() {
            panic!("mbedTLS sources not found in {}: run scripts/fetch-mbedtls.sh or set MBEDTLS_DIR", root.display());
        }
        let crypto = root.join("tf-psa-crypto");

        let mut build = cc::Build::new();
        build
            .include(manifest.join("csrc"))
            .include(root.join("include"))
            .include(crypto.join("include"))
            .include(crypto.join("drivers/builtin/include"))
            .include(root.join("library"))
            .include(crypto.join("core"))
            .include(crypto.join("drivers/builtin/src"))
            .include(crypto.join("dispatch"))
            .include(crypto.join("extras"))
            .include(crypto.join("platform"))
            .include(crypto.join("utilities"))
            .define("MBEDTLS_CONFIG_FILE", "\"tiny_hc_mbedtls_config.h\"")
            .define("TF_PSA_CRYPTO_CONFIG_FILE", "\"tiny_hc_crypto_config.h\"")
            .warnings(false)
            .file(manifest.join("csrc/tiny_hc_tls.c"));
        // Everything outside the enabled configuration compiles to nothing.
        for dir in [
            root.join("library"),
            crypto.join("core"),
            crypto.join("drivers/builtin/src"),
            crypto.join("extras"),
            crypto.join("platform"),
            crypto.join("utilities"),
        ] {
            add_sources(&mut build, &dir);
        }
        build.compile("mbedtls");

        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            // BCryptGenRandom, used for entropy.
            println!("cargo:rustc-link-lib=bcrypt");
        }
    }

    fn add_sources(build: &mut cc::Build, dir: &Path) {
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "c"))
            .collect();
        files.sort();
        build.files(files);
    }
}
