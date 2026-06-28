use anyhow::{Context, Result};
use serde::Deserialize;
use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

fn main() -> Result<()> {
    let target = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.183:53317".to_string());
    let url = format!("http://{target}/peer/handshake");
    println!("target={target}");
    println!("url={url}");

    let bind_first = env::args().any(|arg| arg == "--bind-first");
    let _listener = if bind_first {
        let listener = TcpListener::bind("0.0.0.0:53319").context("bind 0.0.0.0:53319")?;
        println!("bind_first=0.0.0.0:53319 ok");
        Some(listener)
    } else {
        None
    };

    let addr: SocketAddr = target.parse().context("parse IPv4 socket address")?;
    probe_tcp(addr)?;
    probe_manual_http(addr)?;
    probe_ureq(&url)?;
    probe_reqwest(&url)?;
    Ok(())
}

fn probe_tcp(addr: SocketAddr) -> Result<()> {
    print!("tcp_connect... ");
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5));
    match stream {
        Ok(_) => {
            println!("ok");
            Ok(())
        }
        Err(err) => {
            println!("error={err:?}");
            Err(err).context("TcpStream::connect_timeout failed")
        }
    }
}

fn probe_manual_http(addr: SocketAddr) -> Result<()> {
    print!("manual_http... ");
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .context("manual HTTP connect failed")?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(b"GET /peer/handshake HTTP/1.1\r\nHost: probe\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let first = response.lines().next().unwrap_or("<empty>");
    println!("{first}");
    Ok(())
}

fn probe_ureq(url: &str) -> Result<()> {
    print!("ureq... ");
    let response = ureq::get(url).timeout(Duration::from_secs(5)).call();
    match response {
        Ok(response) => {
            let status = response.status();
            let device: PublicDevice = response.into_json()?;
            println!("status={status} device={} port={}", device.device_name, device.port);
            Ok(())
        }
        Err(err) => {
            println!("error={err:?}");
            Err(err).context("ureq failed")
        }
    }
}

#[derive(Debug, Deserialize)]
struct PublicDevice {
    device_name: String,
    port: u16,
}

fn probe_reqwest(url: &str) -> Result<()> {
    print!("reqwest_blocking... ");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let response = client.get(url).send();
    match response {
        Ok(response) => {
            println!("status={}", response.status());
            Ok(())
        }
        Err(err) => {
            println!("error={err:?}");
            Err(err).context("reqwest failed")
        }
    }
}
