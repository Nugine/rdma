dev:
    just fmt    
    cargo clippy
    cargo test

fmt:
    cargo fmt
    cargo sort -w > /dev/null

doc:
    cargo doc --no-deps --open

sync-version:
    #!/bin/bash -e
    cd {{justfile_directory()}}
    vers='0.1.0-dev'
    for pkg in `ls crates`
    do
        echo $pkg $vers
        pushd crates/$pkg > /dev/null
        cargo set-version $vers
        popd > /dev/null
    done
