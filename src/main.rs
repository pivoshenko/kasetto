//! Module that contains the `kasetto` binary entrypoint.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> kasetto::Result<()> {
    kasetto::run()
}
