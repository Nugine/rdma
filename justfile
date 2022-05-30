dev:
    just fmt    
    cargo clippy
    cargo test
    cargo build --release

fmt:
    cargo fmt
    cargo sort -w > /dev/null

doc:
    cargo doc --no-deps --open

sync-version:
    #!/bin/bash -e
    cd {{justfile_directory()}}
    vers='0.2.0-dev'
    echo $vers
    for pkg in `fd --glob '*' -t d -d 1 ./crates ./examples`
    do
        echo $pkg
        pushd $pkg > /dev/null
        cargo set-version $vers
        popd > /dev/null
    done

install-examples:
    cargo install --offline --path examples/rdma-devices
    cargo install --offline --path examples/rdma-pingpong

udeps:
    cargo udeps --workspace --all-features

bench-pingpong:
    #!/bin/bash -ex
    ibv_rc_pingpong -g 2 -e &
    sleep 0.1
    ibv_rc_pingpong -g 2 -e 127.0.0.1
    sleep 0.1
    export RUST_LOG=warn
    rdma-rcpp &
    sleep 0.1
    rdma-rcpp 127.0.0.1
