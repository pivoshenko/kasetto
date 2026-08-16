//! Module that contains the `kst` binary entrypoint, the short alias for `kasetto`.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::process::ExitCode {
    kasetto::run()
}
