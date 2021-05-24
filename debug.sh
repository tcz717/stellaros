ROOT="/home/tcz/stellaros"
cargo xbuild --target=./aarch64-unknown-none.json -q && \
cp "$ROOT/target/aarch64-unknown-none/debug/stellaros" "$ROOT/bigbang/stellaros" && \
cd "$ROOT/bigbang" && \
cargo xbuild --target ../aarch64-unknown-none.json -q