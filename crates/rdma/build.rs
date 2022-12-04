#![deny(clippy::all)]

use std::env;
use std::path::PathBuf;

fn link_rdma_core(lib_name: &str, pkg_name: &str, version: &str, include_paths: &mut Vec<String>) {
    let result: _ = pkg_config::Config::new()
        .atleast_version(version)
        .statik(false)
        .probe(lib_name);

    let lib = result.unwrap_or_else(|_| panic!("please install {pkg_name} {version})"));
    println!("found {pkg_name} {}", lib.version);

    for path in lib.include_paths {
        let path = path.to_str().expect("non-utf8 path");
        include_paths.push(path.to_owned());
    }
}

fn main() {
    if cfg!(docsrs) || env::var("DOCS_RS").is_ok() {
        return;
    }

    let mut include_paths: Vec<String> = Vec::new();

    {
        let lib_name = "libibverbs";
        let pkg_name = "libibverbs-dev";
        let version = "1.14.41";
        link_rdma_core(lib_name, pkg_name, version, &mut include_paths);
    }

    {
        let lib_name = "librdmacm";
        let pkg_name = "librdmacm-dev";
        let version = "1.3.41";
        link_rdma_core(lib_name, pkg_name, version, &mut include_paths);
    }

    {
        include_paths.sort_unstable();
        include_paths.dedup_by(|x, first| x == first);
        include_paths.push("/usr/include".into());
        println!("include paths: {include_paths:?}");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    {
        let include_args = include_paths.iter().map(|p| format!("-I{p}"));

        let bindings = bindgen::Builder::default()
            .clang_args(include_args)
            .header("src/bindings/generated.h")
            .allowlist_function("ibv.+")
            .allowlist_type("ibv.+")
            .allowlist_var("IBV.+")
            .allowlist_var("_RS.+")
            .allowlist_type("verbs.+")
            .allowlist_function("_ibv_query_gid_ex")
            .allowlist_function("rdma.+")
            .allowlist_type("rdma.+")
            .blocklist_type("pthread.+")
            .blocklist_type("__pthread.+")
            .blocklist_type("timespec")
            .blocklist_type("socklen_t")
            .blocklist_function("ibv_reg_mr")
            .blocklist_function("ibv_query_port")
            .prepend_enum_name(false)
            .default_enum_style("consts".parse().unwrap())
            .size_t_is_usize(true)
            .rustfmt_bindings(true)
            .rust_target("1.47".parse().unwrap());

        {
            let mut cmd_flags = bindings.command_line_flags();
            for flag in &mut cmd_flags {
                let s = format!("{flag:?}");
                *flag = s;
            }

            println!("bindgen {}", cmd_flags.join(" "));
        }

        bindings
            .generate()
            .expect("Unable to generate bindings")
            .write_to_file(out_dir.join("generated.rs"))
            .expect("Couldn't write bindings!");
    }
}
