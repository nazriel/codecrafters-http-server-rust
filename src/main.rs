use itertools::Itertools;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

enum Method {
    Get,
}

impl std::convert::From<&str> for Method {
    fn from(input: &str) -> Self {
        match input.to_lowercase().as_str() {
            "get" => Method::Get,
            _ => Method::Get,
        }
    }
}

struct Request {
    path: String,
}

impl Request {
    async fn parse(buff: &mut BufReader<&mut TcpStream>) -> anyhow::Result<Self> {
        let mut line = String::new();
        buff.read_line(&mut line).await?;
        let (_method, path, _version) = line
            .split_whitespace()
            .collect_tuple()
            .expect("invalid first HTTP line");
        Ok(Self { path: path.into() })
    }
}

async fn handle_connection(mut stream: TcpStream) -> anyhow::Result<()> {
    println!("accepted new connection");

    let mut b = BufReader::new(&mut stream);
    let req = Request::parse(&mut b).await?;

    if req.path == "/" {
        stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    } else {
        stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await?;
    }

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
                eprintln!("error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
