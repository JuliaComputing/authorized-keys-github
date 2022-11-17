extern crate clap;
#[macro_use]
extern crate fstrings;
extern crate reqwest;
extern crate tempfile;
extern crate tokio;
extern crate users;
use clap::App;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::io::Write;
use std::process;
use std::time::SystemTime;
use tempfile::NamedTempFile;
use users::get_user_by_uid;

static GITHUB_URL: &str = "https://github.com";
static DEFAULT_KEYS_DIRECTORY: &str = "/var/keys";

fn uid_keys_file(keys_dir: &str, uid: u32) -> String {
    return f!("{keys_dir}/{uid}.keys");
}

async fn refresh_keys_from_github(
    uid: u32,
    keys_dir: &str,
    keyfile_contents: &mut String,
) -> Result<File, String> {
    eprintln!("Refreshing keys from github");
    let user = get_user_by_uid(uid);
    if user.is_none() {
        panic!("Username for uid {} not found", uid);
    }

    let user = user.unwrap();
    let username = user.name().to_str();
    if username.is_none() {
        panic!("{}", "Failed to get username");
    }

    let username = username.unwrap();
    let response_future = reqwest::get(&f!("{GITHUB_URL}/{username}.keys")).await;
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
    keyfile_contents.clone_from(&body.unwrap());

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

#[tokio::main]
async fn main() {
    let matches = App::new("authorized-keys-github")
        .version("0.1")
        .author("Keno Fischer <keno@juliacomputing.com>")
        .about("Retrieve SSH keys from GitHub auth local caching")
        .args_from_usage(
            "--fp=[fp]        'The fingerprint for the requested key'
             --keys-dir=[kd]  'The keys directory (default: /var/keys)'
             <uid>            'The UID for which to retrieve authorized keys'",
        )
        .get_matches();
    let uid = matches.value_of("uid");
    let keys_dir_arg = matches.value_of("keys-dir");
    let mut requested_fp = matches.value_of("fp");
    if !uid.is_some() {
        eprintln!("ERROR: uid is required");
        process::exit(1);
    }
    let uid = uid.unwrap().parse::<u32>();
    if !uid.is_ok() {
        eprintln!("ERROR: uid must be an integer");
        process::exit(1);
    }
    let uid = uid.ok().unwrap();
    if uid < 1000 {
        eprintln!("ERROR: UID must be > 1000");
        process::exit(1);
    }

    if requested_fp.is_some() {
        let unwrap_requested_fp = requested_fp.unwrap();
        let separator = unwrap_requested_fp.find(':').unwrap();
        assert_eq!(unwrap_requested_fp.get(..separator).unwrap(), "SHA256");
        requested_fp = Some(unwrap_requested_fp.get(separator + 1..).unwrap());
    }

    let keys_dir: &str;
    if keys_dir_arg.is_some() {
        keys_dir = keys_dir_arg.unwrap();
    } else {
        keys_dir = DEFAULT_KEYS_DIRECTORY;
    }

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
        _ = refresh_keys_from_github(uid, keys_dir, &mut keyfile_contents).await;
        _ = print_requested_key(&keyfile_contents, requested_fp);
    } else {
        let _read = file.unwrap().read_to_string(&mut keyfile_contents);
        if !match print_requested_key(&keyfile_contents, requested_fp) {
            Ok(ok) => ok,
            Err(_) => false,
        } {
            _ = refresh_keys_from_github(uid, keys_dir, &mut keyfile_contents).await;
            _ = print_requested_key(&keyfile_contents, requested_fp);
        }
    }
    process::exit(0);
}
