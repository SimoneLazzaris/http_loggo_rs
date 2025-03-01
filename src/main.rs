use crate::htpasswd::Htpasswd;
use clap::Parser;
use file_rotate::{ContentLimit, FileRotate, TimeFrequency, compression::*, suffix::AppendCount};
mod htpasswd;
use http_auth_basic::Credentials;
use std::fs;
use std::io::BufWriter;
use std::io::Cursor;
use std::io::Write;
use std::str::FromStr;
use tiny_http::{Request, Response, Server};

#[derive(Parser)]
struct Cfg {
    #[arg(short, long, default_value = "127.0.0.1")]
    address: String,
    #[arg(short, long, default_value_t = 8080)]
    port: u16,
    #[arg(short, long, default_value = "/tmp/logfile")]
    logfile: String,
    #[arg(short, long, default_value_t = 30)]
    rotate: usize,
    #[arg(short = 'H', long, default_value = "")]
    htpasswd: String,
}

fn get_content_type(request: &Request) -> String {
    let ct = tiny_http::HeaderField::from_str("content-type").unwrap();
    for h in request.headers() {
        if h.field == ct {
            return h.value.to_string();
        }
    }
    return "*/*".to_string();
}

fn writelog(jbody: &serde_json::Value, logfile: &mut dyn Write) {
    if jbody.is_array() {
        for elem in jbody.as_array().unwrap() {
            let res = writeln!(logfile, "{}", elem.to_string());
            if res.is_err() {
                println!("Error writing to log file: {}", res.unwrap_err());
            }
        }
    } else {
        let res = writeln!(logfile, "{}", jbody.to_string());
        if res.is_err() {
            println!("Error writing to log file: {}", res.unwrap_err());
        }
    }
}

fn authenticate(request: &Request, pwd_dict: &Option<Htpasswd>) -> bool {
    if pwd_dict.is_none() {
        return true;
    }
    let hh = tiny_http::HeaderField::from_str("authorization").unwrap();
    let auth_f = request.headers().iter().position(|r| r.field == hh);
    if auth_f.is_none() {
        return false;
    }
    let auth_idx = auth_f.unwrap();
    let authheader = request.headers()[auth_idx].value.as_str();

    if let Ok(credentials) = Credentials::from_header((&authheader).to_string()) {
        println!("cred: {:?}", credentials);
        return pwd_dict
            .as_ref()
            .expect("baba")
            .check(&credentials.user_id, &credentials.password);
    }
    return false;
}

fn process_request<W: Write>(
    request: &mut Request,
    logfile: &mut W,
    pwd_dict: &Option<Htpasswd>,
) -> Response<Cursor<Vec<u8>>> {
    if request.url() == "/health" {
        let response = Response::from_string("OK\n").with_status_code(200);
        return response;
    }
    println!(
        "{} - {} {}",
        request.remote_addr(),
        request.method(),
        request.url(),
    );
    if request.method().as_str() != "POST" {
        let response = Response::from_string("Bad method\n").with_status_code(400);
        return response;
    }
    if !authenticate(request, pwd_dict) {
        let response = Response::from_string("401 Unauthorized\n").with_status_code(401);
        return response;
    }
    let mut content = String::new();
    request.as_reader().read_to_string(&mut content).unwrap();
    if get_content_type(&request) == "application/json" {
        let jconv = serde_json::from_str(&content);
        if jconv.is_ok() {
            let jbody: serde_json::Value = jconv.unwrap();
            writelog(&jbody, logfile)
        } else {
            println!("JSON parse error: {}", jconv.unwrap_err());
        }
    } else {
        let res = writeln!(logfile, "UNKNOWN: {}", content);
        if res.is_err() {
            println!("Error writing to log file: {}", res.unwrap_err());
        }
    }
    let response = Response::from_string("OK\n");
    return response;
}

fn main() {
    let cfg = Cfg::parse();
    let htpwd_content: String;
    let pwd_dict: Option<Htpasswd> = if !cfg.htpasswd.is_empty() {
        htpwd_content =
            fs::read_to_string(cfg.htpasswd).expect("Should have been able to read the file");
        Some(Htpasswd::from(htpwd_content.as_str()))
    } else {
        None
    };

    let url = format!("{address}:{port}", address = cfg.address, port = cfg.port);
    println!("Starting server on {}", url);

    let server = Server::http(url).unwrap();
    let logfile = FileRotate::new(
        cfg.logfile,
        AppendCount::new(cfg.rotate),
        ContentLimit::Time(TimeFrequency::Daily),
        Compression::OnRotate(1),
        #[cfg(unix)]
        None,
    );
    let mut stream = BufWriter::new(logfile);
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]);
    for mut request in server.incoming_requests() {
        let response = process_request(&mut request, &mut stream, &pwd_dict);
        let _ = request.respond(response);
    }
    let res = stream.flush();
    if res.is_err() {
        println!("Error flushing log file: {}", res.unwrap_err());
    }
}
