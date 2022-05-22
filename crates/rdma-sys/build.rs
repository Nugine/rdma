#![deny(clippy::all)]

use std::env;
use std::path::PathBuf;

fn main() {
    {
        let lib_name = "libibverbs";
        let pkg_name = "libibverbs-dev";
        let version = "1.8.28";

        let result: _ = pkg_config::Config::new()
            .atleast_version(version)
            .statik(true)
            .probe(lib_name);

        match result {
            Ok(lib) => println!("Found  {pkg_name} {}", lib.version),
            Err(_) => panic!("Please install {pkg_name}"),
        }
    }

    {
        let lib_name = "librdmacm";
        let pkg_name = "librdmacm-dev";
        let version = "1.2.28";

        let result: _ = pkg_config::Config::new()
            .atleast_version(version)
            .statik(true)
            .probe(lib_name);

        match result {
            Ok(lib) => println!("Found  {pkg_name} {}", lib.version),
            Err(_) => panic!("Please install {pkg_name}"),
        }
    }

    let bindings = bindgen::Builder::default()
        .header("/usr/include/infiniband/verbs.h")
        .header("/usr/include/rdma/rdma_cma.h")
        .header("/usr/include/rdma/rdma_verbs.h")
        .allowlist_function("ibv.+")
        .allowlist_type("ibv.+")
        .allowlist_function("rdma.+")
        .allowlist_type("rdma.+")
        .blocklist_type("pthread.+")
        .blocklist_type("__pthread.+")
        .blocklist_type("timespec")
        .blocklist_type("socklen_t")
        // .blocklist_type("in_addr")
        // .blocklist_type("in_addr_t")
        // .blocklist_type("in_port_t")
        // .blocklist_type("in6_addr.*")
        // .blocklist_type("sa_family_t")
        // .blocklist_type("sockaddr.*")
        // .blocklist_type("__u.+")
        .prepend_enum_name(false)
        .default_enum_style("consts".parse().unwrap())
        .bitfield_enum("ibv_.+_flags")
        .size_t_is_usize(true)
        .rustfmt_bindings(true)
        .rust_target("1.47".parse().unwrap())
        .generate()
        .expect("Unable to generate bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
