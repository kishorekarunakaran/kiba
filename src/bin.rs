use kiva::{HashStore, Store};
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, PartialEq)]
enum Request {
    Ping,
    Get { key: String },
    Set { key: String, val: String },
    NoOp,
    Invalid { error: String },
}

#[derive(Debug, PartialEq)]
struct Response {
    body: String,
}

#[derive(Debug)]
struct Message {
    req: Request,
    pipe: oneshot::Sender<Response>,
}

#[derive(Debug, PartialEq)]
enum Token {
    Ping,
    Get,
    Set,
    Operand(String),
}

#[derive(Debug)]
struct ParserError(String);

async fn parse_tokens(tokens: Vec<Token>) -> Result<Request, ParserError> {
    let argc = tokens.len();
    if argc == 0 {
        return Ok(Request::NoOp);
    }
    let op = &tokens[0];
    match op {
        Token::Ping => {
            if argc != 1 {
                return Err(ParserError(format!(
                    "Ping op expected no operands, got {}",
                    argc - 1
                )));
            }
            return Ok(Request::Ping);
        }
        Token::Get => {
            if argc != 2 {
                return Err(ParserError(format!(
                    "Get op expected exactly 1 operand, got {}",
                    argc - 1
                )));
            }
            match &tokens[1] {
                Token::Operand(k) => {
                    return Ok(Request::Get { key: k.to_string() });
                }
                _ => return Err(ParserError(format!("Get operands cannot be op types"))),
            }
        }
        Token::Set => {
            if argc != 3 {
                return Err(ParserError(format!(
                    "Set op expected 2 operands, got {}",
                    argc - 1
                )));
            }
            let key;
            match &tokens[1] {
                Token::Operand(k) => key = k.to_string(),
                _ => return Err(ParserError(format!("Set operands cannot be op types"))),
            }
            let val;
            match &tokens[2] {
                Token::Operand(v) => val = v.to_string(),
                _ => return Err(ParserError(format!("Set operands cannot be op types"))),
            }
            return Ok(Request::Set { key: key, val: val });
        }
        _ => return Err(ParserError(format!("Invalid op token"))),
    }
}

async fn tokenize(bytes: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let text = std::str::from_utf8(bytes).unwrap();
    let mut chunks = text
        .split(|c: char| c.is_whitespace() || c == '\u{0}')
        .filter(|s| !s.is_empty());

    while let Some(chunk) = chunks.next() {
        match chunk.to_uppercase().as_str() {
            "PING" => tokens.push(Token::Ping),
            "GET" => tokens.push(Token::Get),
            "SET" => tokens.push(Token::Set),
            _ => tokens.push(Token::Operand(chunk.to_string())),
        }
    }
    tokens
}

async fn parse_request(bytes: &[u8]) -> Result<Request, ParserError> {
    let tokens = tokenize(bytes).await;
    let req = parse_tokens(tokens).await?;
    println!("{:?}", req);
    Ok(req)
}

async fn exec_request(req: Request, store: &mut HashStore<String, String>) -> Response {
    match req {
        Request::Ping => {
            return Response {
                body: "PONG".to_string(),
            }
        }
        Request::Get { key } => match store.get(&key).unwrap() {
            Some(val) => {
                return Response {
                    body: format!("\"{}\"", val),
                }
            }
            None => {
                return Response {
                    body: "(nil)".to_string(),
                }
            }
        },
        Request::Set { key, val } => {
            let _ = store.set(key, val);
            return Response {
                body: "OK".to_string(),
            };
        }
        Request::NoOp => {
            return Response {
                body: "\u{0}".to_string(),
            }
        }
        Request::Invalid { error } => {
            return Response {
                body: format!("ERROR: {}", error),
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================");
    println!("Kiva Server (v0.1)");
    println!("==================");

    let cbuf = 100;
    let (tx, mut rx) = mpsc::channel(cbuf);

    let _manager = tokio::spawn(async move {
        let mut store: HashStore<String, String> = Store::new();
        println!("** Initialized data store");

        while let Some(msg) = rx.recv().await {
            let msg: Message = msg; // Make type of `msg` explicit to compiler
            let resp = exec_request(msg.req, &mut store).await;
            let _ = msg.pipe.send(resp);
        }
    });

    let url = "127.0.0.1:6464";
    let mut listener = TcpListener::bind(url).await?;
    println!("** Listening on: {}", url);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!(
            "** Successfully established inbound TCP connection with: {}",
            &addr
        );
        let mut txc = tx.clone();
        let _task = tokio::spawn(async move {
            loop {
                let mut buf = [0; 128];
                let _ = socket.read(&mut buf[..]).await;

                let req;
                match parse_request(&buf).await {
                    Ok(request) => req = request,
                    Err(e) => {
                        req = Request::Invalid {
                            error: e.0.to_string(),
                        }
                    }
                }

                let (send_pipe, recv_pipe) = oneshot::channel();
                let msg = Message {
                    req: req,
                    pipe: send_pipe,
                };

                let _ = txc.send(msg).await;

                let resp = recv_pipe.await.unwrap();
                let _ = socket.write_all(resp.body.as_bytes()).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiva::{HashStore, Store};

    #[tokio::test]
    async fn test_tokenize() {
        assert_eq!(tokenize(b"PING    ").await, vec![Token::Ping]);
        assert_eq!(
            tokenize("SET foo bar\u{0}\u{0}\u{0}".as_bytes()).await,
            vec![
                Token::Set,
                Token::Operand("foo".to_string()),
                Token::Operand("bar".to_string())
            ]
        );
        assert_eq!(
            tokenize(b"  GET    baz       ").await,
            vec![Token::Get, Token::Operand("baz".to_string())]
        );
        assert_eq!(
            tokenize(b" set time now").await,
            vec![
                Token::Set,
                Token::Operand("time".to_string()),
                Token::Operand("now".to_string()),
            ]
        );
        assert_eq!(
            tokenize(b"is invalid request").await,
            vec![
                Token::Operand("is".to_string()),
                Token::Operand("invalid".to_string()),
                Token::Operand("request".to_string())
            ]
        );
        assert_eq!(tokenize(b" ").await, vec![]);
    }

    #[tokio::test]
    async fn test_valid_parse_tokens() {
        assert_eq!(
            parse_tokens(vec![Token::Ping]).await.unwrap(),
            Request::Ping
        );
        assert_eq!(
            parse_tokens(vec![Token::Get, Token::Operand("foo".to_string())])
                .await
                .unwrap(),
            Request::Get {
                key: "foo".to_string()
            }
        );
        assert_eq!(
            parse_tokens(vec![
                Token::Set,
                Token::Operand("foo".to_string()),
                Token::Operand("bar".to_string())
            ])
            .await
            .unwrap(),
            Request::Set {
                key: "foo".to_string(),
                val: "bar".to_string()
            }
        );
        assert_eq!(parse_tokens(vec![]).await.unwrap(), Request::NoOp);
    }

    #[tokio::test]
    #[should_panic]
    async fn test_invalid_ping() {
        parse_tokens(vec![Token::Ping, Token::Operand("foo".to_string())])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn test_invalid_get() {
        parse_tokens(vec![Token::Get]).await.unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn test_invalid_set() {
        parse_tokens(vec![
            Token::Set,
            Token::Operand("baz".to_string()),
            Token::Operand("bar".to_string()),
            Token::Operand("foo".to_string()),
        ])
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_response() {
        let mut store: HashStore<String, String> = Store::new();
        assert_eq!(
            exec_request(Request::Ping, &mut store).await,
            Response {
                body: "PONG".to_string()
            }
        )
    }
}
