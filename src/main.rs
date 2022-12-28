use clap::Parser;
use std::io::Cursor;
use std::str::FromStr;
use tiny_http::{Request, Response, Server};
use file_rotate::{FileRotate, ContentLimit, suffix::AppendCount, compression::Compression, TimeFrequency};
use std::io::Write;

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
            let _ = writeln!(logfile, "{}", elem.to_string());
        }
    } else {
        let _ = writeln!(logfile, "{}", jbody.to_string());
    }
}


fn process_request(request: &mut Request, logfile: &mut dyn Write) -> Response<Cursor<Vec<u8>>> {
    println!("{} - {} {}", request.remote_addr(), request.method(), request.url(), );
    let mut content = String::new();
    request.as_reader().read_to_string(&mut content).unwrap();
    if get_content_type(&request) == "application/json" {
        let jconv = serde_json::from_str(&content);
        if jconv.is_ok() {
            let jbody: serde_json::Value = jconv.unwrap();
            writelog(&jbody, logfile)
        }
    } else {
        let _ = writeln!(logfile, "UNKOWN: {}", content);
    }
    let response = Response::from_string("hello world\n");
    return response;
}

fn main() {
    let cfg = Cfg::parse();
    let url = format!("{address}:{port}", address = cfg.address, port = cfg.port);
    println!("Starting server on {}", url);

    let server = Server::http(url).unwrap();
    let mut logfile = FileRotate::new(
        cfg.logfile, 
        AppendCount::new(cfg.rotate), 
        ContentLimit::Time(TimeFrequency::Hourly),
        Compression::None, 
        #[cfg(unix)]
        None,
    );    
    for mut request in server.incoming_requests() {
        let response = process_request(&mut request, &mut logfile);
        let _ = request.respond(response);
    }
}
