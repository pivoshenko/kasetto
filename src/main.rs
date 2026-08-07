#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> kasetto::Result<()> {
    kasetto::run()
}
