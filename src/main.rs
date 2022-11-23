extern crate clap;
extern crate reqwest;
extern crate tempfile;
extern crate tokio;
extern crate users;
use clap::{crate_authors, crate_name, crate_version, App};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process;
use std::time::SystemTime;
use tempfile::NamedTempFile;
use users::{get_current_uid, get_user_by_uid};

static GITHUB_URL: &str = "https://github.com";
static DEFAULT_KEYS_DIRECTORY: &str = "/var/keys";

fn uid_keys_file(keys_dir: &str, uid: u32) -> String {
    return format!("{keys_dir}/{uid}.keys");
}

fn get_usernames(
    uid: &u32,
    overrides: HashMap<u32, Vec<String>>,
) -> Result<Vec<String>, Box<dyn Error>> {
    // If we have overrides for the given `uid`, return those usernames
    if overrides.contains_key(uid) {
        return Ok(overrides.get(uid).unwrap().clone());
    }

    // Otherwise, return the one username we get from the system
    let username = match get_user_by_uid(*uid) {
        None => Err(format!("Username for uid '{}' not found", uid))?,
        Some(user) => match user.name().to_str() {
            Some(username) => username.to_string(),
            None => Err(format!("Username for uid '{}' unwrappable", uid))?,
        },
    };

    return Ok(vec![username]);
}

async fn refresh_keys_from_github(
    uid: u32,
    usernames: &Vec<String>,
    keys_dir: &str,
    keyfile_contents: &mut String,
) -> Result<File, String> {
    for username in usernames {
        eprintln!("Refreshing keys from github for user {username}");
        let response_future = reqwest::get(&format!("{GITHUB_URL}/{username}.keys")).await;
        if response_future.is_err() {
            panic!("Request to GitHub failed: {}", response_future.unwrap_err());
        }

        let response = response_future.ok().unwrap();
        if !response.status().is_success() {
            return Err(format!(
                "Remote had invalid request code: {}",
                response.status()
            ));
        }

        let body = response.text().await;
        if body.is_err() {
            return Err(format!("Failed to retrieve body: {}", body.unwrap_err()));
        }
        keyfile_contents.push_str(&body.unwrap());
    }

    let mut file = match NamedTempFile::new_in(keys_dir) {
        Ok(file) => file,
        Err(e) => return Err(format!("Failed to create temp file: {:?}", e)),
    };

    let written = file.write_all(keyfile_contents.as_bytes());
    if written.is_err() {
        return Err(format!("Failed to write key: {}", written.unwrap_err()));
    }

    return match file.persist(uid_keys_file(keys_dir, uid)) {
        Ok(mut file) => {
            let _ = &file.seek(SeekFrom::Start(0));
            Ok(file)
        }
        Err(e) => return Err(e.to_string()),
    };
}

fn print_requested_key(
    keyfile_contents: &str,
    requested_fp: Option<&str>,
) -> std::io::Result<bool> {
    if !requested_fp.is_some() {
        print!("{}", keyfile_contents);
        return Ok(true);
    } else {
        let requested_fp = requested_fp.unwrap();
        let lines = keyfile_contents.lines();
        for line in lines {
            if line.starts_with("#") || line.is_empty() {
                continue;
            }
            let pubkey = sshkeys::PublicKey::from_string(&line).unwrap();
            let fp = pubkey.fingerprint();
            if requested_fp == fp.hash {
                println!("{}", line);
                return Ok(true);
            }
        }
        return Ok(false);
    }
}

fn is_outdated(time: SystemTime) -> bool {
    // If we haven't checked this file in 15 minutes, check it now,
    // the key may have been revoked
    return match SystemTime::now().duration_since(time) {
        Ok(duration) => duration.as_secs() > 15 * 60,
        Err(_) => true,
    };
}

fn load_overrides_file(path: &str) -> Result<HashMap<u32, Vec<String>>, Box<dyn Error>> {
    let mut mapping = HashMap::new();
    let file = File::open(path)?;
    let meta = file.metadata()?;
    let curr_uid = get_current_uid();
    let mode = meta.permissions().mode();
    if (meta.uid() != curr_uid) || (mode & 0o022 != 0) {
        Err(format!("Must be owned by current user (uid: {curr_uid}) and not writable by anyone else (permissions: {:o})", mode))?;
    }
    for line in BufReader::new(file).lines() {
        if let Ok(l) = line {
            if l.starts_with("#") || l.is_empty() {
                continue;
            }
            let uid_names: Vec<_> = l.split(":").collect();
            let uid_str = uid_names[0];
            let uid: u32 = match uid_str.parse() {
                Ok(uid) => uid,
                Err(e) => Err(format!("Invalid UID '{uid_str}': {}", e))?,
            };
            let names: Vec<String> = uid_names[1]
                .split_whitespace()
                .map(|s| s.to_owned())
                .collect();

            // Error out if the overrides file contains a duplicate mapping for a UID
            if mapping.contains_key(&uid) {
                Err(format!("Duplicate mapping for UID '{uid}'"))?;
            }
            mapping.insert(uid, names);
        }
    }
    return Ok(mapping);
}

fn parse_uids(uids_str: Option<clap::Values>) -> Result<HashSet<u32>, Box<dyn Error>> {
    let mut uids = HashSet::new();
    if uids_str.is_some() {
        for uid_str in uids_str.unwrap() {
            let uid = match uid_str.parse() {
                Ok(uid) => uid,
                Err(e) => Err(format!("Invalid UID '{uid_str}': {}", e))?,
            };
            uids.insert(uid);
        }
    }
    return Ok(uids);
}

#[tokio::main]
async fn main() {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about("Retrieve SSH keys from GitHub auth with local caching")
        .args_from_usage(
            "--fp=[fp]                     'The fingerprint for the requested key'
             --keys-dir=[kd]               'The keys directory (default: /var/keys)'
             --overrides-file=[af]          'The path to an overrides file (default: none)'
             --allow-system-uid=[uid]...   'Enable operation for a specific system UID (<1000)'
             <uid>                         'The UID for which to retrieve authorized keys'",
        )
        .get_matches();

    // If we've been given an override file, read it in, otherwise just use an empty mapping
    let overrides_file = matches.value_of("overrides-file");
    let overrides_mapping = match overrides_file {
        None => HashMap::new(),
        Some(path) => match load_overrides_file(path) {
            Ok(overrides_mapping) => overrides_mapping,
            // If we tried to get an override mapping, but it failed in some way, panic.
            Err(e) => {
                eprintln!("ERROR: Invalid overrides file {path}: {}", e);
                process::exit(1);
            }
        },
    };

    let allowed_system_uids = match parse_uids(matches.values_of("allow-system-uid")) {
        Ok(uids) => uids,
        Err(e) => {
            eprintln!("ERROR: Unable to parse system UID: {}", e);
            process::exit(1);
        }
    };

    let uid = match matches.value_of("uid") {
        Some(uid) => uid.parse().expect("ERROR: uid must be an integer"),
        None => {
            eprintln!("ERROR: uid is required");
            process::exit(1);
        }
    };
    if uid < 1000 && !allowed_system_uids.contains(&uid) {
        eprintln!("ERROR: UID must be > 1000");
        process::exit(1);
    }

    // Determine the usernames we're going to query
    let usernames = match get_usernames(&uid, overrides_mapping) {
        Ok(usernames) => usernames,
        Err(e) => {
            eprintln!("Unable to determine username(s) for {uid}: {}", e);
            process::exit(1);
        }
    };

    // Only allow SHA256 fingerprints
    let mut requested_fp = matches.value_of("fp");
    if requested_fp.is_some() {
        let unwrap_requested_fp = requested_fp.unwrap();
        let separator = unwrap_requested_fp.find(':').unwrap();
        assert_eq!(unwrap_requested_fp.get(..separator).unwrap(), "SHA256");
        requested_fp = Some(unwrap_requested_fp.get(separator + 1..).unwrap());
    }

    // Parse out the keys directory
    let keys_dir = match matches.value_of("keys-dir") {
        Some(keys_dir) => keys_dir,
        None => DEFAULT_KEYS_DIRECTORY,
    };

    let mut keyfile_contents = String::new();
    let file = File::open(uid_keys_file(keys_dir, uid));
    if !file.is_ok()
        || match &file.as_ref().ok().unwrap().metadata() {
            Ok(md) => match md.modified() {
                Ok(date) => is_outdated(date),
                Err(_) => false,
            },
            Err(_) => false,
        }
    {
        _ = refresh_keys_from_github(uid, &usernames, keys_dir, &mut keyfile_contents).await;
        _ = print_requested_key(&keyfile_contents, requested_fp);
    } else {
        let _read = file.unwrap().read_to_string(&mut keyfile_contents);
        if !match print_requested_key(&keyfile_contents, requested_fp) {
            Ok(ok) => ok,
            Err(_) => false,
        } {
            _ = refresh_keys_from_github(uid, &usernames, keys_dir, &mut keyfile_contents).await;
            _ = print_requested_key(&keyfile_contents, requested_fp);
        }
    }
    process::exit(0);
}
