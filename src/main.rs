use std::{
    env, fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
};

const DEFAULT_ADDR: &str = "0.0.0.0:3021";
const LEADERBOARD_FILE: &str = "leaderboard.tsv";
const MAX_LEADERBOARD_ENTRIES: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
struct LeaderboardEntry {
    score: i32,
    moves: u32,
    date: String,
}

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
    let path = target.split('?').next().unwrap_or("/");

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    if path == "/api/leaderboard" {
        return handle_leaderboard_api(&mut stream, method, &body);
    }

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

fn handle_leaderboard_api(stream: &mut TcpStream, method: &str, body: &[u8]) -> io::Result<()> {
    match method {
        "GET" | "HEAD" => {
            let entries = read_leaderboard()?;
            let json = leaderboard_json(&entries);
            send_response(
                stream,
                "200 OK",
                "application/json; charset=utf-8",
                &json,
                method,
            )
        }
        "POST" => match parse_leaderboard_entry(body) {
            Some(entry) => {
                let mut entries = read_leaderboard()?;
                entries.push(entry);
                sort_leaderboard(&mut entries);
                entries.truncate(MAX_LEADERBOARD_ENTRIES);
                write_leaderboard(&entries)?;
                let json = leaderboard_json(&entries);
                send_response(
                    stream,
                    "200 OK",
                    "application/json; charset=utf-8",
                    &json,
                    method,
                )
            }
            None => send_response(
                stream,
                "400 Bad Request",
                "text/plain; charset=utf-8",
                b"Bad leaderboard entry",
                method,
            ),
        },
        _ => send_response(
            stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method not allowed",
            method,
        ),
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

fn read_leaderboard() -> io::Result<Vec<LeaderboardEntry>> {
    let text = match fs::read_to_string(LEADERBOARD_FILE) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut entries = text
        .lines()
        .filter_map(|line| parse_leaderboard_line(line.as_bytes()))
        .collect::<Vec<_>>();
    sort_leaderboard(&mut entries);
    entries.truncate(MAX_LEADERBOARD_ENTRIES);
    Ok(entries)
}

fn write_leaderboard(entries: &[LeaderboardEntry]) -> io::Result<()> {
    let mut text = String::new();
    for entry in entries {
        text.push_str(&format!(
            "{}\t{}\t{}\n",
            entry.score,
            entry.moves,
            sanitize_date(&entry.date)
        ));
    }
    fs::write(LEADERBOARD_FILE, text)
}

fn parse_leaderboard_entry(body: &[u8]) -> Option<LeaderboardEntry> {
    parse_leaderboard_line(body)
}

fn parse_leaderboard_line(line: &[u8]) -> Option<LeaderboardEntry> {
    let text = std::str::from_utf8(line).ok()?.trim();
    let mut parts = text.splitn(3, '\t');
    let score = parts.next()?.parse().ok()?;
    let moves = parts.next()?.parse().ok()?;
    let date = sanitize_date(parts.next()?);

    Some(LeaderboardEntry { score, moves, date })
}

fn sanitize_date(date: &str) -> String {
    date.chars()
        .filter(|ch| !matches!(ch, '\t' | '\n' | '\r'))
        .take(64)
        .collect()
}

fn sort_leaderboard(entries: &mut [LeaderboardEntry]) {
    entries.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.moves.cmp(&b.moves))
            .then_with(|| a.date.cmp(&b.date))
    });
}

fn leaderboard_json(entries: &[LeaderboardEntry]) -> Vec<u8> {
    let mut json = String::from("[");
    for (idx, entry) in entries.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"score\":{},\"moves\":{},\"date\":\"{}\"}}",
            entry.score,
            entry.moves,
            json_escape(&entry.date)
        ));
    }
    json.push(']');
    json.into_bytes()
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
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

    #[test]
    fn leaderboard_entries_parse_from_tsv() {
        assert_eq!(
            parse_leaderboard_entry(b"120\t42\t2026-07-07T12:00:00.000Z"),
            Some(LeaderboardEntry {
                score: 120,
                moves: 42,
                date: "2026-07-07T12:00:00.000Z".to_owned(),
            })
        );
        assert!(parse_leaderboard_entry(b"bad\t42\tdate").is_none());
    }

    #[test]
    fn leaderboard_sorts_by_score_then_moves() {
        let mut entries = vec![
            LeaderboardEntry {
                score: 100,
                moves: 40,
                date: "2026-07-07T12:00:01Z".to_owned(),
            },
            LeaderboardEntry {
                score: 150,
                moves: 60,
                date: "2026-07-07T12:00:02Z".to_owned(),
            },
            LeaderboardEntry {
                score: 150,
                moves: 30,
                date: "2026-07-07T12:00:03Z".to_owned(),
            },
        ];

        sort_leaderboard(&mut entries);

        assert_eq!(entries[0].score, 150);
        assert_eq!(entries[0].moves, 30);
        assert_eq!(entries[1].score, 150);
        assert_eq!(entries[1].moves, 60);
        assert_eq!(entries[2].score, 100);
    }

    #[test]
    fn leaderboard_json_escapes_dates() {
        let json = leaderboard_json(&[LeaderboardEntry {
            score: 10,
            moves: 2,
            date: "date \"quoted\"".to_owned(),
        }]);

        assert_eq!(
            String::from_utf8(json).unwrap(),
            r#"[{"score":10,"moves":2,"date":"date \"quoted\""}]"#
        );
    }
}
