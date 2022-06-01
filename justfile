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
    vers='0.4.0-dev'
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
    cargo install --offline --path examples/rdma-async

udeps:
    cargo udeps --workspace --all-features

bench-pingpong-rc:
    #!/bin/bash -ex
    ibv_rc_pingpong -g 2 -s 1024 &
    sleep 0.1
    ibv_rc_pingpong -g 2 -s 1024 127.0.0.1
    sleep 0.1
    export RUST_LOG=warn
    rdma-pingpong rc &
    sleep 0.1
    rdma-pingpong rc 127.0.0.1

bench-pingpong-ud:
    #!/bin/bash -ex
    ibv_ud_pingpong -g 2 -s 1024 &
    sleep 0.1
    ibv_ud_pingpong -g 2 -s 1024 127.0.0.1
    sleep 0.1
    export RUST_LOG=warn
    rdma-pingpong ud &
    sleep 0.1
    rdma-pingpong ud 127.0.0.1
