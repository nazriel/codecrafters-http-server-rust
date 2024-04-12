use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

async fn handle_connection(mut stream: TcpStream) -> anyhow::Result<()> {
    println!("accepted new connection");
    let mut b = [0u8; 1024];
    let _ = stream.read(&mut b).await?;
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221").await?;

    loop {
        let conn = listener.accept().await;
        match conn {
            Ok((stream, _)) => {
                tokio::spawn(async move { handle_connection(stream).await });
            }
            Err(e) => {
                println!("error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
