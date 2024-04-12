use itertools::Itertools;
use std::{collections::HashMap, env, path::PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

#[derive(PartialEq)]
enum Method {
    Get,
    Post,
    Put,
}

impl std::convert::From<&str> for Method {
    fn from(input: &str) -> Self {
        match input.to_lowercase().as_str() {
            "get" => Method::Get,
            "post" => Method::Post,
            "put" => Method::Put,
            e => panic!("unknown method {e}"),
        }
    }
}

struct Request {
    path: String,
    headers: std::collections::HashMap<String, String>,
    method: Method,
}

impl Request {
    async fn parse(buff: &mut BufReader<&mut TcpStream>) -> anyhow::Result<Self> {
        let mut line = String::new();
        buff.read_line(&mut line).await?;
        let (method, path, _version) = line
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
            method: method.into(),
        })
    }
}

struct Response<'a> {
    body: String,
    status: u16,
    stream: &'a mut TcpStream,
    headers: HashMap<String, String>,
}
impl<'a> Response<'a> {
    fn new(stream: &'a mut TcpStream) -> Self {
        Self {
            body: String::new(),
            status: 200,
            stream,
            headers: HashMap::new(),
        }
    }

    fn status(&mut self, status: u16) -> &mut Self {
        self.status = status;
        self
    }

    fn body(&mut self, body: &str) -> &mut Self {
        self.body = body.into();
        self
    }

    fn header(&mut self, key: &str, value: &str) -> &mut Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    async fn send(&mut self) -> anyhow::Result<()> {
        let status_text = match self.status {
            200 => "200 OK",
            404 => "404 Not Found",
            500 => "500 Internal Server Error",
            _ => "500 Internal Server Error",
        };
        self.stream
            .write_all(format!("HTTP/1.1 {status_text}\r\n").as_bytes())
            .await?;

        for (hname, hvalue) in &self.headers {
            self.stream
                .write_all(format!("{hname}: {hvalue}\r\n").as_bytes())
                .await?;
        }

        if !self.body.is_empty() {
            self.stream
                .write_all(format!("Content-Length: {}\r\n\r\n", self.body.len()).as_bytes())
                .await?;
            self.stream.write_all(self.body.as_bytes()).await?;
        } else {
            self.stream.write_all(b"Content-Length: 0\r\n\r\n").await?;
        }
        Ok(())
    }
}

async fn handle_connection(mut stream: TcpStream, config: Config) -> anyhow::Result<()> {
    println!("accepted new connection");

    let mut b = BufReader::new(&mut stream);
    let req = Request::parse(&mut b).await?;

    if req.path == "/" {
        Response::new(&mut stream).status(200).send().await?;
    } else if req.path == "/user-agent" {
        let ua = req.headers.get("User-Agent").cloned().unwrap_or_default();

        Response::new(&mut stream)
            .status(200)
            .header("Content-Type", "text/plain")
            .body(&ua)
            .send()
            .await?;
    } else if req.path.starts_with("/echo/") {
        let payload = req
            .path
            .strip_prefix("/echo/")
            .expect("some payload should exist");

        Response::new(&mut stream)
            .status(200)
            .header("Content-Type", "text/plain")
            .body(payload)
            .send()
            .await?;
    } else if req.path.starts_with("/files/") {
        if config.static_files.is_none() {
            return Response::new(&mut stream).status(404).send().await;
        }
        let entity = req
            .path
            .strip_prefix("/files/")
            .expect("some entity path should exist");

        let fpath = format!(
            "{}/{entity}",
            config.static_files.unwrap().to_str().unwrap()
        );
        if !tokio::fs::try_exists(&fpath).await? {
            return Response::new(&mut stream).status(404).send().await;
        }
        match tokio::fs::read_to_string(fpath).await {
            Ok(body) => {
                return Response::new(&mut stream)
                    .status(200)
                    .header("Content-Type", "application/octet-stream")
                    .body(&body)
                    .send()
                    .await;
            }
            Err(err) => {
                return Response::new(&mut stream)
                    .status(500)
                    .body(format!("failed to read file: {err}").as_str())
                    .send()
                    .await;
            }
        }
    } else {
        Response::new(&mut stream).status(404).send().await?;
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct Config {
    address: String,
    port: u16,
    static_files: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config {
        address: "127.0.0.1".into(),
        port: 4221,
        static_files: None,
    };
    let listener = TcpListener::bind(format!("{}:{}", config.address, config.port)).await?;
    let mut it = env::args_os();
    while let Some(arg) = it.next() {
        if arg == "--directory" {
            config.static_files = it.next().map(|x| x.into());
        }
    }

    println!("Running with following config: {:?}", &config);

    loop {
        let conn = listener.accept().await;
        match conn {
            Ok((stream, _)) => {
                let cfg = config.clone();
                tokio::spawn(async move { handle_connection(stream, cfg).await });
            }
            Err(e) => {
                eprintln!("error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
