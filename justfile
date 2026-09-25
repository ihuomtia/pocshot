# Pocshot build recipes
#
#  Linux native:              cargo (Arch `rust`)
#  Windows cross (from Linux): rustup + toolchain 1.98.0 with std x86_64-pc-windows-gnu,
#                              plus mingw-w64 gcc (`sudo pacman -S mingw-w64-gcc`)
#  Windows native:             rust GNU toolchain (`rustup default stable-x86_64-pc-windows-gnu`)

rustup := env_var("HOME") + "/.cargo/bin/cargo"
tc     := "1.98.0-x86_64-unknown-linux-gnu"
win    := "x86_64-pc-windows-gnu"

default: build

# Release build for the current platform (Linux or Windows native)
build:
    cargo build --release -p pocshot

# Debug build for the current platform
build-debug:
    cargo build -p pocshot

# Release build of the software-rendering flavor (`pocshot-soft`): wgpu + a CPU
# adapter, no OpenGL. For GPU-less machines (RDP sessions, GPU-less VMs).
build-soft:
    cargo build --release -p pocshot --no-default-features --features software
    cp target/release/pocshot target/release/pocshot-soft

# Cross-compile release .exe for Windows (from Linux)
[linux]
build-win:
    {{rustup}} +{{tc}} build --release --target {{win}} -p pocshot

# Cross-compile the software-rendering .exe for Windows (from Linux). The copy
# keeps `pocshot-soft.exe` around even if a later `build-win` overwrites
# `pocshot.exe`.
[linux]
build-win-soft:
    {{rustup}} +{{tc}} build --release --target {{win}} -p pocshot --no-default-features --features software
    cp target/{{win}}/release/pocshot.exe target/{{win}}/release/pocshot-soft.exe

# Cross-compile debug .exe for Windows (from Linux)
[linux]
build-win-debug:
    CARGO_INCREMENTAL=0 {{rustup}} +{{tc}} build --target {{win}} -p pocshot

# Compile the GUI test harness for the Windows target (run them on Windows)
[linux]
test-win:
    CARGO_INCREMENTAL=0 {{rustup}} +{{tc}} build --target {{win}} --tests -p pocshot-gui

# Remove all build artifacts (native + cross)
clean:
    cargo clean

# Remove only Windows cross-compile artifacts (keeps native builds)
clean-win:
    rm -rf target/{{win}}

# Print where the binaries land
bin:
    @printf 'native:  target/release/pocshot (add .exe on Windows)\n'
    @printf 'soft:    target/release/pocshot-soft (linux)\n'
    @printf 'windows: target/{{win}}/release/pocshot.exe\n'
    @printf 'win soft: target/{{win}}/release/pocshot-soft.exe\n'