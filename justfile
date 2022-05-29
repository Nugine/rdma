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
    vers='0.1.2-dev'
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
    cargo install --offline --path examples/rdma-rcpp
