//! Run `cargo build -p serviceless-flatten-so` first, then
//! `cargo run -p serviceless-dynamic --example load_flatten -- target/debug`.
use serviceless_dynamic::{DynamicMessage, SrvDynLoader};

#[repr(C)]
#[derive(Clone, Copy)]
struct Add {
    amount: u64,
}

unsafe impl DynamicMessage for Add {
    type Result = u64;
    const NAME: &'static str = "Add";
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/debug".into());
    let path = std::path::Path::new(&directory)
        .join(libloading::library_filename("serviceless_flatten_so"));
    let loader = SrvDynLoader::new().isolated_runtime(true);
    let service = unsafe { loader.load_lib("S1", path)? };
    assert_eq!(service.call(Add { amount: 2 }).await?, 2);
    service.send(Add { amount: 4 })?;
    assert_eq!(service.call(Add { amount: 3 }).await?, 9);
    service.close().await;
    println!("flatten SO loaded and called successfully");
    Ok(())
}
