//! Fetch the overall timing of each job as a whole for every AppVeyor build.
//!
//! Usage:
//!
//! ```sh
//! cargo run --bin top `cat token.txt` [previous-build-number]
//! ```

extern crate chrono;
extern crate regex;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use std::env::args;
use reqwest::{Client, Url};
use reqwest::header::{Accept, Authorization, Bearer, Headers};
use regex::{Regex, RegexSet};
use chrono::{DateTime, Utc};
use std::time::Duration;
use std::thread::sleep;
use std::collections::HashMap;

macro_rules! api {
    ($endpoint:expr) => {
        concat!("https://ci.appveyor.com/api/projects/rust-lang/rust", $endpoint)
    }
}

macro_rules! regex_set {
    ($($regex_index:expr),+) => {
        $(1 << $regex_index)|+
    }
}


#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct History {
    builds: Vec<Build>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Jobs {
    build: Build,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Build {
    build_id: u64,
    version: String,
    started: String,
    message: String,
    status: String,
    jobs: Vec<Job>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Job {
    name: String,
    started: DateTime<Utc>,
    finished: DateTime<Utc>,
}

fn main() {
    let pr_number_regex = Regex::new("Auto merge of #([0-9]+)").unwrap();
    let job_names_regex_set = RegexSet::new(&[
        r"--build=x86_64-pc-windows-msvc\b", // 0
        r"--build=i686-pc-windows-msvc\b",   // 1
        r"--build=x86_64-pc-windows-gnu\b",  // 2
        r"--build=i686-pc-windows-gnu\b",    // 3
        r"\bcheck-aux\b",                    // 4
        r"\bcargotest\b",                    // 5
        r"\bpython x.py test\b",             // 6
        r"\bpython x.py dist\b",             // 7
        r"\bDEPLOY_ALT=1\b",                 // 8
    ]).unwrap();

    let mut regex_set_to_job_index = HashMap::with_capacity(11);
    regex_set_to_job_index.insert(regex_set![0, 6], 0); // check-64-msvc
    regex_set_to_job_index.insert(regex_set![1, 6], 1); // check-32-msvc
    regex_set_to_job_index.insert(regex_set![0, 4], 2); // check-aux
    regex_set_to_job_index.insert(regex_set![0, 5, 6], 3); // cargotest
    regex_set_to_job_index.insert(regex_set![3, 6], 4); // check-32-gnu
    regex_set_to_job_index.insert(regex_set![2, 6], 5); // check-64-gnu
    regex_set_to_job_index.insert(regex_set![0, 7], 6); // dist-64-msvc
    regex_set_to_job_index.insert(regex_set![1, 7], 7); // dist-32-msvc
    regex_set_to_job_index.insert(regex_set![3, 7], 8); // dist-32-gnu
    regex_set_to_job_index.insert(regex_set![2, 7], 9); // dist-64-gnu
    regex_set_to_job_index.insert(regex_set![0, 7, 8], 10); // dist-alt

    let mut args = args();
    args.next();
    let token = args.next().unwrap();

    let mut default_headers = Headers::new();
    default_headers.set(Authorization(Bearer { token }));
    default_headers.set(Accept::json());

    let client = Client::builder()
        .default_headers(default_headers)
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();

    // Obtain the list of build IDs
    let mut params = vec![("recordsNumber", "100".to_owned())];
    if let Some(start_build_id) = args.next() {
        params.push(("startBuildId", start_build_id));
    }
    let history_url = Url::parse_with_params(api!("/history"), params).unwrap();

    let mut response = client.get(history_url).send().unwrap();
    println!(
        "Build ID\tBuild number\tPR number\tStart time\t\
         check-64-msvc\tcheck-32-msvc\tcheck-aux\tcargotest\tcheck-32-gnu\tcheck-64-gnu\t\
         dist-64-msvc\tdist-32-msvc\tdist-32-gnu\tdist-64-gnu\tdist-alt"
    );

    let history = response.json::<History>().unwrap();

    let jobs_base_url = Url::parse(api!("/build/")).unwrap();
    for build in history.builds {
        if build.status != "success" {
            continue;
        }
        let pr_number = pr_number_regex
            .captures(&build.message)
            .and_then(|captures| captures.get(1))
            .map(|m| m.as_str())
            .unwrap_or("?????");
        print!(
            "{build_id}\t{build_number}\t{pr_number}\t{start_time}",
            build_id = build.build_id,
            build_number = build.version,
            pr_number = pr_number,
            start_time = build.started,
        );

        let jobs_url = jobs_base_url.join(&build.version).unwrap();
        let raw_jobs = client.get(jobs_url).send().unwrap().json::<Jobs>().unwrap();
        let mut durations = [-1; 11];
        for job in raw_jobs.build.jobs {
            let seconds_elapsed = job.finished.timestamp() - job.started.timestamp();
            let regex_index_set = job_names_regex_set
                .matches(&job.name)
                .into_iter()
                .map(|x| 1 << x)
                .sum();
            if let Some(job_index) = regex_set_to_job_index.get(&regex_index_set) {
                durations[*job_index] = seconds_elapsed;
            }
        }
        for duration in &durations {
            print!("\t{}", duration);
        }
        println!();

        sleep(Duration::from_millis(250));
    }
}
