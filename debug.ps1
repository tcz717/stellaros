cargo xbuild --target=.\aarch64-unknown-none.json -q
Copy-Item "F:\Code\Science\stellaros\target\aarch64-unknown-none\debug\stellaros" "F:\Code\Science\stellaros\bigbang\stellaros"
Set-Location "F:\Code\Science\stellaros\bigbang"
cargo xbuild --target ..\aarch64-unknown-none.json -q
Set-Location "C:\Program Files\qemu"
qemu-system-aarch64 -s -S -machine virt -m 1024M -cpu cortex-a53 -semihosting -nographic -kernel "F:\Code\Science\stellaros\bigbang\target\aarch64-unknown-none\debug\stellaros-bigbang" -d int,mmu -display none