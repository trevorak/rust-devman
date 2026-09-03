use std::fs::{File};
use std::io;

pub fn download_remote_file(url: &str, out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut response = reqwest::blocking::get(url)?;

    let mut dest = File::create(out_path)?;

    io::copy(&mut response, &mut dest)?;

    Ok(())
}
