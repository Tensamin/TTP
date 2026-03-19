pub mod bin;
pub mod ui;
use tokio::io::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // 99.7 % Vibe Coded
    loop {
        ui::run_comm_to_binary_converter()?;
        bin::run_binary_to_comm_converter()?;
    }
}
