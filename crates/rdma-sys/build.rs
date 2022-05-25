#![deny(clippy::all)]

use std::env;
use std::path::PathBuf;

fn main() {
    let mut include_paths: Vec<String> = Vec::new();

    {
        let lib_name = "libibverbs";
        let pkg_name = "libibverbs-dev";
        let version = "1.14.41";

        let result: _ = pkg_config::Config::new()
            .atleast_version(version)
            .statik(true)
            .probe(lib_name);

        let lib = result.unwrap_or_else(|_| panic!("please install {pkg_name} {version})"));
        println!("found {pkg_name} {}", lib.version);

        for p in lib.include_paths {
            let p = p.to_str().expect("utf8 path").to_owned();
            include_paths.push(p)
        }
    }

    {
        let lib_name = "librdmacm";
        let pkg_name = "librdmacm-dev";
        let version = "1.3.41";

        let result: _ = pkg_config::Config::new()
            .atleast_version(version)
            .statik(true)
            .probe(lib_name);

        let lib = result.unwrap_or_else(|_| panic!("please install {pkg_name} {version})"));
        println!("found {pkg_name} {}", lib.version);

        for p in lib.include_paths {
            let p = p.to_str().expect("utf8 path").to_owned();
            include_paths.push(p)
        }
    }
    {
        include_paths.sort_unstable();
        include_paths.dedup_by(|x, first| x == first);
        include_paths.push("/usr/include".into());
        println!("include paths: {:?}", include_paths);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    {
        let include_args = include_paths.iter().map(|p| format!("-I{}", p));

        let bindings = bindgen::Builder::default()
            .clang_args(include_args)
            .header("src/bindings.h")
            .allowlist_function("ibv.+")
            .allowlist_type("ibv.+")
            .allowlist_function("rdma.+")
            .allowlist_type("rdma.+")
            .blocklist_type("pthread.+")
            .blocklist_type("__pthread.+")
            .blocklist_type("timespec")
            .blocklist_type("socklen_t")
            .blocklist_function("ibv_query_port")
            .prepend_enum_name(false)
            .default_enum_style("consts".parse().unwrap())
            .bitfield_enum("ibv_.+_flags")
            .bitfield_enum("ibv_.+_mask")
            .size_t_is_usize(true)
            .rustfmt_bindings(true)
            .rust_target("1.47".parse().unwrap())
            .generate()
            .expect("Unable to generate bindings");

        bindings
            .write_to_file(out_dir.join("bindings.rs"))
            .expect("Couldn't write bindings!");
    }

    {
        let file = "src/rsrdma.c";
        let lib = "rsrdma";

        cc::Build::new()
            .file(file)
            .includes(&include_paths)
            .compile(lib);

        println!("cargo:rustc-link-lib=static={}", lib);
    }
}
