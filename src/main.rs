use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
};

const DEFAULT_ADDR: &str = "0.0.0.0:3021";

fn main() -> io::Result<()> {
    let addr = env::var("SOLITAIRE_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_owned());
    let listener = TcpListener::bind(&addr)?;
    println!("Solitaire listening on http://{addr}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_client(stream) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");

    if method != "GET" && method != "HEAD" {
        return send_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"Method not allowed",
            method,
        );
    }

    let Some(path) = request_path(target) else {
        return send_response(
            &mut stream,
            "400 Bad Request",
            "text/plain",
            b"Bad request",
            method,
        );
    };

    match fs::read(&path) {
        Ok(body) => send_response(&mut stream, "200 OK", content_type(&path), &body, method),
        Err(error) if error.kind() == io::ErrorKind::NotFound => send_response(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"Not found",
            method,
        ),
        Err(error) => {
            eprintln!("could not read {}: {error}", path.display());
            send_response(
                &mut stream,
                "500 Internal Server Error",
                "text/plain",
                b"Internal server error",
                method,
            )
        }
    }
}

fn request_path(target: &str) -> Option<PathBuf> {
    let path = target.split('?').next().unwrap_or("/");
    let path = if path == "/" { "/index.html" } else { path };
    let relative = path.strip_prefix('/')?;
    let mut resolved = PathBuf::from(".");

    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            _ => return None,
        }
    }

    Some(resolved)
}

fn send_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    method: &str,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;

    if method != "HEAD" {
        stream.write_all(body)?;
    }

    stream.flush()
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_serves_index() {
        assert_eq!(request_path("/").unwrap(), PathBuf::from("./index.html"));
    }

    #[test]
    fn request_path_rejects_parent_traversal() {
        assert!(request_path("/../Cargo.toml").is_none());
    }

    #[test]
    fn wasm_uses_wasm_content_type() {
        assert_eq!(
            content_type(Path::new("dist/solitaire.wasm")),
            "application/wasm"
        );
    }
}
