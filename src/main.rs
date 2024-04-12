use itertools::Itertools;
use std::collections::HashMap;
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
    headers: std::collections::HashMap<String, String>,
}

impl Request {
    async fn parse(buff: &mut BufReader<&mut TcpStream>) -> anyhow::Result<Self> {
        let mut line = String::new();
        buff.read_line(&mut line).await?;
        let (_method, path, _version) = line
            .split_whitespace()
            .collect_tuple()
            .expect("invalid first HTTP line");

        let mut headers = HashMap::new();
        let mut line = String::new();

        while let Ok(n) = buff.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            if line == "\r\n" {
                break;
            }
            let (hname, hvalue) = line.split_once(": ").expect("invalid header line");
            let hvalue = hvalue.trim();
            headers.insert(hname.to_string(), hvalue.to_string());
            line.clear();
        }

        Ok(Self {
            path: path.into(),
            headers,
        })
    }
}

async fn handle_connection(mut stream: TcpStream) -> anyhow::Result<()> {
    println!("accepted new connection");

    let mut b = BufReader::new(&mut stream);
    let req = Request::parse(&mut b).await?;

    if req.path == "/" {
        stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    } else if req.path.starts_with("/echo/") {
        let payload = req
            .path
            .strip_prefix("/echo/")
            .expect("some payload should exist");

        stream.write_all(b"HTTP/1.1 200 OK\r\n").await?;
        stream.write_all(b"Content-Type: text/plain\r\n").await?;
        stream
            .write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())
            .await?;

        stream.write_all(payload.as_bytes()).await?;
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
