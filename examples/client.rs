use std::io::prelude::*;
use tokio::net::TcpStream;
use tokio::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================");
    println!("Kiva Client (v0.1)");
    println!("==================");

    let url = "127.0.0.1:6464";
    let mut stream = TcpStream::connect(url).await?;

    println!("** Successfully established outbound TCP connection");
    println!("** Listening on: {}", url);

    loop {
        let mut wbuf = String::new();
        print!("> ");
        std::io::stdout().flush().unwrap();
        std::io::stdin()
            .read_line(&mut wbuf)
            .expect("Failed to read input");
        stream.write_all(wbuf.as_bytes()).await?;

        let mut rbuf = [0; 128];
        stream.read(&mut rbuf[..]).await?;

        println!("{}", String::from_utf8_lossy(&rbuf));
    }
}
