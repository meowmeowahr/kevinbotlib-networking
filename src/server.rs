use std::error::Error;
use tokio::net::TcpListener;

pub async fn start_server(host: &str, port: u16) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("{}:{}", host, port)).await?;
    println!("Server running on {}:{}", host, port);
    loop {
        let (_socket, addr) = listener.accept().await?;
        println!("New connection: {}", addr);
        // Handle client connections
    }
}

pub fn stop_server() -> Result<(), Box<dyn Error>> {
    println!("Server stopped");
    Ok(())
}