#![allow(unused_imports, unused_variables)]
pub use controller::*;
use tracing::*;

#[tokio::main]
async fn main() -> Result<()> {
    let (_manager, controller) = Manager::new().await;
    tokio::select! {
        _ = controller => warn!("Controller exited"),
    }
    Ok(())
}
