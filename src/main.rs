use atty::Stream;
use chrono::{DateTime, Local, Utc};
use clap::Parser;
use clio::ClioPath;
use expanduser::expanduser;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use timediff::*;
use unicode_segmentation::UnicodeSegmentation;

use anyhow::{Context, Result};
use pager::Pager;
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Opts {
    /// Path to directory
    #[clap(value_parser = clap::value_parser!(ClioPath).exists().is_dir(), default_value = ".")]
    directory: ClioPath,

    /// Number of files to display
    /// Default: 10
    #[clap(short, long, default_value = "10")]
    num_files: usize,
}

fn get_relative_time(t: SystemTime) -> Result<String> {
    let duration = SystemTime::now()
        .duration_since(t)
        .context("Unable to get duration since")?;
    // turn to negative duration str
    let duration = format!("-{}s", duration.as_secs());
    // use locale
    Ok(TimeDiff::to_diff(duration).parse()?)
}
fn human_readable_system_time(t: SystemTime) -> String {
    // Convert SystemTime to chrono DateTime
    let datetime: DateTime<Utc> = DateTime::<Utc>::from(t);
    // Convert the UTC time to local time
    let local_time: DateTime<Local> = datetime.with_timezone(&Local);

    // Format the time as "[hour]:[minute]:[second] [day]-[month repr:short]-[year]"
    local_time.format("%H:%M:%S %d-%b-%Y").to_string()
}

fn abbreviate_filename(filename: &str, max_length: usize) -> String {
    let graphemes: Vec<&str> = filename.graphemes(true).collect();
    if graphemes.len() > max_length {
        let half_length = max_length / 2;
        let start = &graphemes[0..half_length].concat();
        let end = &graphemes[graphemes.len() - half_length..].concat();
        format!("{}...{}", start, end)
    } else {
        filename.to_string()
    }
}

fn list_dir(path: &Path, num_files: &usize) -> Result<()> {
    let path_str = path.to_str().expect("Unable to convert path to string");
    let path = expanduser(path_str)?;
    let entries = path.read_dir().expect("Failed to read directory");

    let mut file_info: Vec<(PathBuf, SystemTime)> = entries
        .map(|entry| {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();

            // skiop hidden files and symbolic links
            if path.file_name().unwrap().to_str().unwrap().starts_with(".") || path.is_symlink() {
                return (path, SystemTime::UNIX_EPOCH);
            }

            let metadata = fs::metadata(&path).expect("Unable to read metadata");
            let modified_time = metadata.modified().expect("Unable to get modified time");

            (path, modified_time)
        })
        .collect();

    // sort by modified time
    file_info.sort_by_key(|&(_, modified)| modified);
    file_info.reverse();

    // slice by num_files
    file_info.truncate(*num_files);

    if !atty::is(Stream::Stdout) {
        print_file_info(file_info)?;
        return Ok(());
    }
    println!(
        "\x1b[1mDisplaying {} most recently modified files for  {} \x1b[22m",
        file_info.len(),
        path.display()
    );
    print_file_info(file_info)?;
    Ok(())
}

fn print_file_info(file_info: Vec<(PathBuf, SystemTime)>) -> Result<()> {
    // Get the longest filename length for aligning columns
    let max_filename_length = 80;
    // Max length for filename before abbreviating

    // Pretty print the files and directories in "ls" style
    for (path, modified_time) in file_info {
        let hr_time = human_readable_system_time(modified_time);
        let relative_time = get_relative_time(modified_time)?;
        let filename = path.file_name().unwrap().to_string_lossy();
        let filename_abbreviated = abbreviate_filename(&filename, max_filename_length);
        let metadata = fs::metadata(&path).expect("Unable to read metadata");

        if metadata.is_dir() {
            // Print directories in blue with relative time

            let is_tty = atty::is(Stream::Stdout);
            if is_tty {
                // Print directories in blue with relative time
                println!(
                    "\x1b[34m{:<max$}\x1b[0m  {}  ({})",
                    filename_abbreviated,
                    hr_time,
                    relative_time,
                    max = max_filename_length
                );
            } else {
                // Print directories without color
                println!(
                    "{:<max$}  {}  ({})",
                    filename_abbreviated,
                    hr_time,
                    relative_time,
                    max = max_filename_length
                );
            }
        } else {
            // Print files normally with relative time
            println!(
                "{:<max$}  {}  ({})",
                filename_abbreviated,
                hr_time,
                relative_time,
                max = max_filename_length
            );
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    if &opts.num_files > &20 {
        // setup the pager
        let mut pager = Pager::new();
        pager.setup();
    }
    let res = list_dir(&opts.directory, &opts.num_files);
    match res {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
