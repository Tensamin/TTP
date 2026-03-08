pub mod bin;
pub mod ui;

#[tokio::main]
async fn main() {
    // 99.8 % Vibe Coded
    ui::run_comm_to_binary_converter();
    bin::run_binary_to_comm_converter();
}
