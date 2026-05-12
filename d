#!/usr/bin/env bash

set -ex

cd "$(dirname "$0")"

CMD=$1
shift

case "$CMD" in
    download-all)
        echo TODO "$CMD"
    ;;
    install-chiptool)
        cargo install --git https://github.com/embassy-rs/chiptool
    ;;
    extract-all)
        peri=$1
        shift
        echo "$@"

        rm -rf "tmp/$peri"
        mkdir -p "tmp/$peri"

        for f in `ls svd/ch32*`; do
            echo "$f"
            svd_path="$f"
            f="${f#"svd/ch32"}"
            f="${f%".svd.patched"}"
            echo -n "processing $f ..."

            trans_args=()
            if test -f "transforms/$peri.yaml"; then
                trans_args=(--transform "transforms/$peri.yaml")
            fi

            if chiptool extract-peripheral "${trans_args[@]}" --svd "$svd_path" --peripheral "$peri" "$@" > "tmp/$peri/$f.yaml" 2> "tmp/$peri/$f.err"; then
                rm "tmp/$peri/$f.err"
                echo OK
            else
                if grep -q 'peripheral not found' "tmp/$peri/$f.err"; then
                    echo No Peripheral
                else
                    echo OTHER FAILURE
                fi
                rm "tmp/$peri/$f.yaml"
            fi
        done
    ;;
    gen)
        rm -rf build/data
        echo "TODO: More chips to be added"
        cargo run -p ch32-data-gen && cargo run -p ch32-metapac-gen -- "CH32X03*" "CH32V*" "CH32L*" "CH32M*" CH641 CH643
    ;;
    dump-memory-x)
        # Render memory.x for inspection, into build/memory-x-dump/<chip>.memory.x.
        # No --features: dump every (option x split-subset) variant per chip.
        # With --features: dump the single memory.x produced by that feature set.
        # Usage:
        #   ./d dump-memory-x [CHIP|GLOB]...
        #   ./d dump-memory-x CH32V203RBT6 --features memory-option-c160_r32,memory-usr-split
        cargo run -p ch32-metapac-gen --bin dump-memory-x -- "$@"
    ;;
    ci)
        echo TODO "$CMD"
    ;;
    *)
        echo "unknown command"
    ;;
esac

