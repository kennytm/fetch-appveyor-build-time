//! Analyze the log file to extract timing breakdown for each significant step
//! in an AppVeyor job.
//!
//! Usage:
//!
//! ```sh
//! cargo run --bin log log.txt
//! ```

extern crate regex;

use std::env::args_os;
use std::io::{BufReader, BufRead};
use std::fs::File;
use regex::Regex;

fn main() {
    let timing_regex = Regex::new(r"\b([0-9]+\.[0-9]+) secs\b|\bfinished in ([0-9]+\.[0-9]+)\b").unwrap();
    let header_regexes = [
        (
            Regex::new(r"Attempting with retry: make prepare").unwrap(),
            "make-prepare",
        ),
        (
            Regex::new(r"Doctest: bootstrap").unwrap(),
            "pytest/bootstrap",
        ),
        (
            Regex::new(r"Building stage(\d) compiler artifacts").unwrap(),
            "stage$1-rustc",
        ),
        (
            Regex::new(r"Building rustdoc for stage(\d)").unwrap(),
            "stage$1-rustdoc",
        ),
        (
            Regex::new(r"Building stage(\d) ([\w-]+) artifacts").unwrap(),
            "stage$1-$2",
        ),
        (
            Regex::new(r"Building stage(\d) tool ([\w-]+)").unwrap(),
            "stage$1-$2",
        ),
        (
            Regex::new(r"Compiling bootstrap v0\.0\.0").unwrap(),
            "bootstrap",
        ),
        (
            Regex::new(r"Building LLVM").unwrap(),
            "llvm",
        ),
        (
            Regex::new(r"test \[[\w-]+\] ([\w-]+)\\").unwrap(),
            "test/$1",
        ),
        (
            Regex::new(r"Testing ([\w-]+) stage(\d)").unwrap(),
            "stage$2-test-$1",
        ),
        (
            Regex::new(r"Running build\\[^\\]+\\stage\d-([\w-]+)\\").unwrap(),
            "test/lib$1",
        ),
        (
            Regex::new(r"Documenting stage\d ([\w-]+)").unwrap(),
            "doc/$1",
        ),
        (
            Regex::new(r"doc tests for:").unwrap(),
            "test/docs",
        ),
    ];

    let log_path = args_os().nth(1).unwrap();
    let log_file = BufReader::new(File::open(log_path).unwrap());
    let mut current_header = None;
    for (line, line_number) in log_file.lines().zip(1..) {
        let line = line.unwrap();
        if let Some(captures) = timing_regex.captures(&line) {
            let m = captures.iter().skip(1).flat_map(|x| x).next().unwrap();
            match current_header.take() {
                Some(header) => {
                    let parsed_number = m.as_str().parse::<f64>().unwrap();
                    println!("{:28}\t{:7.2}", header, parsed_number);
                    if header == "stage0-linkchecker" {
                        current_header = Some("test/linkchecker".to_owned());
                    }
                }
                None => {
                    eprintln!("\x1b[1mmissing header for timing at line {}\x1b[0m\n{}\n", line_number, line);
                    panic!();
                }
            }
        } else if current_header.is_none() {
            for &(ref regex, replacement) in &header_regexes {
                if let Some(captures) = regex.captures(&line) {
                    let mut expansion = String::new();
                    captures.expand(replacement, &mut expansion);
                    current_header = Some(expansion);
                    break;
                }
            }
        }
    }
}