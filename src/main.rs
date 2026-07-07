use std::{
    env, fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
};

const DEFAULT_ADDR: &str = "0.0.0.0:3021";
const APP_ROOT_ENV: &str = "SOLITAIRE_ROOT";
const LEADERBOARD_FILE_ENV: &str = "SOLITAIRE_LEADERBOARD_FILE";
const LEADERBOARD_FILE: &str = "leaderboard.tsv";
const MAX_LEADERBOARD_ENTRIES: usize = 5;

#[derive(Clone, Debug)]
struct AppPaths {
    public_root: PathBuf,
    leaderboard_file: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LeaderboardEntry {
    score: i32,
    moves: u32,
    date: String,
}

fn main() -> io::Result<()> {
    let addr = env::var("SOLITAIRE_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_owned());
    let paths = AppPaths::from_env()?;
    let listener = TcpListener::bind(&addr)?;
    println!("Solitaire listening on http://{addr}");
    println!("Serving files from {}", paths.public_root.display());
    println!("Leaderboard file: {}", paths.leaderboard_file.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_client(stream, &paths) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }

    Ok(())
}

impl AppPaths {
    fn from_env() -> io::Result<Self> {
        let public_root = env::var_os(APP_ROOT_ENV)
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(env::current_dir)?;
        let leaderboard_file = resolve_leaderboard_path(
            &public_root,
            env::var_os(LEADERBOARD_FILE_ENV).map(PathBuf::from),
        );

        Ok(Self {
            public_root,
            leaderboard_file,
        })
    }
}

fn handle_client(mut stream: TcpStream, paths: &AppPaths) -> io::Result<()> {
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

        if let Some(value) = header_value(trimmed, "content-length") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    if path == "/api/leaderboard" {
        return handle_leaderboard_api(&mut stream, method, &body, &paths.leaderboard_file);
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

    let Some(path) = request_path(target, &paths.public_root) else {
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

fn handle_leaderboard_api(
    stream: &mut TcpStream,
    method: &str,
    body: &[u8],
    leaderboard_file: &Path,
) -> io::Result<()> {
    match method {
        "GET" | "HEAD" => {
            let entries = match read_leaderboard(leaderboard_file) {
                Ok(entries) => entries,
                Err(error) => {
                    eprintln!(
                        "could not read leaderboard {}: {error}",
                        leaderboard_file.display()
                    );
                    return send_response(
                        stream,
                        "500 Internal Server Error",
                        "text/plain; charset=utf-8",
                        b"Could not read leaderboard",
                        method,
                    );
                }
            };
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
                let mut entries = match read_leaderboard(leaderboard_file) {
                    Ok(entries) => entries,
                    Err(error) => {
                        eprintln!(
                            "could not read leaderboard {}: {error}",
                            leaderboard_file.display()
                        );
                        return send_response(
                            stream,
                            "500 Internal Server Error",
                            "text/plain; charset=utf-8",
                            b"Could not read leaderboard",
                            method,
                        );
                    }
                };
                entries.push(entry);
                sort_leaderboard(&mut entries);
                entries.truncate(MAX_LEADERBOARD_ENTRIES);
                if let Err(error) = write_leaderboard(leaderboard_file, &entries) {
                    eprintln!(
                        "could not write leaderboard {}: {error}",
                        leaderboard_file.display()
                    );
                    return send_response(
                        stream,
                        "500 Internal Server Error",
                        "text/plain; charset=utf-8",
                        b"Could not write leaderboard",
                        method,
                    );
                }
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

fn resolve_leaderboard_path(public_root: &Path, configured_path: Option<PathBuf>) -> PathBuf {
    match configured_path {
        Some(path) if path.is_absolute() => path,
        Some(path) => public_root.join(path),
        None => public_root.join(LEADERBOARD_FILE),
    }
}

fn request_path(target: &str, public_root: &Path) -> Option<PathBuf> {
    let path = target.split('?').next().unwrap_or("/");
    let path = if path == "/" { "/index.html" } else { path };
    let relative = path.strip_prefix('/')?;
    let mut resolved = public_root.to_path_buf();

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

fn header_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    let (header_name, value) = header.split_once(':')?;
    if header_name.eq_ignore_ascii_case(name) {
        Some(value)
    } else {
        None
    }
}

fn read_leaderboard(path: &Path) -> io::Result<Vec<LeaderboardEntry>> {
    let text = match fs::read_to_string(path) {
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

fn write_leaderboard(path: &Path, entries: &[LeaderboardEntry]) -> io::Result<()> {
    let mut text = String::new();
    for entry in entries {
        text.push_str(&format!(
            "{}\t{}\t{}\n",
            entry.score,
            entry.moves,
            sanitize_date(&entry.date)
        ));
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut tmp_path = path.to_path_buf();
    tmp_path.set_extension("tmp");
    fs::write(&tmp_path, text)?;
    fs::rename(tmp_path, path)
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
        assert_eq!(
            request_path("/", Path::new("/srv/solitaire")).unwrap(),
            PathBuf::from("/srv/solitaire/index.html")
        );
    }

    #[test]
    fn request_path_rejects_parent_traversal() {
        assert!(request_path("/../Cargo.toml", Path::new("/srv/solitaire")).is_none());
    }

    #[test]
    fn leaderboard_path_defaults_under_public_root() {
        assert_eq!(
            resolve_leaderboard_path(Path::new("/srv/solitaire"), None),
            PathBuf::from("/srv/solitaire/leaderboard.tsv")
        );
    }

    #[test]
    fn leaderboard_path_can_be_configured() {
        assert_eq!(
            resolve_leaderboard_path(
                Path::new("/srv/solitaire"),
                Some(PathBuf::from("data/scores.tsv"))
            ),
            PathBuf::from("/srv/solitaire/data/scores.tsv")
        );
        assert_eq!(
            resolve_leaderboard_path(
                Path::new("/srv/solitaire"),
                Some(PathBuf::from("/var/lib/solitaire/scores.tsv"))
            ),
            PathBuf::from("/var/lib/solitaire/scores.tsv")
        );
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
    fn header_lookup_is_case_insensitive() {
        assert_eq!(
            header_value("content-length: 12", "content-length"),
            Some(" 12")
        );
        assert_eq!(
            header_value("Content-Length: 12", "content-length"),
            Some(" 12")
        );
        assert_eq!(header_value("Host: localhost", "content-length"), None);
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
