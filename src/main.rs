use anyhow::{Context, Result};
use atty::Stream;
use chrono::{DateTime, Local, Utc};
use clap::Parser;
use clio::ClioPath;
use expanduser::expanduser;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use timediff::TimeDiff;
use unicode_segmentation::UnicodeSegmentation;
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

#[derive(PartialEq, Eq, Debug)]
enum FileType {
    File,
    Directory,
    Symlink,
    Hidden,
}

#[derive(Debug)]
struct File {
    name: String,
    modified_time: SystemTime,
    relative_time: String,
    file_type: FileType,
    #[allow(dead_code)]
    path: PathBuf,
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

    let mut file_info: Vec<File> = entries
        .map(|entry| get_path_mtime(entry).unwrap())
        .collect();

    // Sort by modified time
    file_info.sort_by_key(|f| f.modified_time);
    file_info.reverse();

    // Remove symlinks and hidden files
    file_info = file_info
        .into_iter()
        .filter(|f| f.file_type != FileType::Symlink && f.file_type != FileType::Hidden)
        .collect();

    // Slice by num_files
    file_info.truncate(*num_files);

    print_file_info(file_info)?;
    Ok(())
}

fn get_path_mtime(entry: std::result::Result<fs::DirEntry, std::io::Error>) -> Result<File> {
    let entry = entry.expect("Failed to read entry");
    let path = entry.path();
    let metadata = fs::metadata(&path).expect("Unable to read metadata");

    let file_type =
        if metadata.is_file() && !path.file_name().unwrap().to_string_lossy().starts_with(".") {
            FileType::File
        } else if metadata.is_dir() {
            FileType::Directory
        } else if metadata.file_type().is_symlink() {
            FileType::Symlink
        } else if path.file_name().unwrap().to_string_lossy().starts_with(".") {
            FileType::Hidden
        } else {
            FileType::File
        };

    let modified_time = metadata.modified().expect("Unable to get modified time");

    let relative_time = get_relative_time(modified_time).expect("Unable to get relative time");
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    Ok(File {
        name,
        modified_time,
        relative_time,
        file_type,
        path,
    })
}

fn print_file_info(file_info: Vec<File>) -> Result<()> {
    let max_filename_length = 80;
    let is_tty = atty::is(Stream::Stdout);

    let modified_time_width = 20;
    let relative_time_width = 15;

    // Show table headers
    if is_tty {
        println!(
            "\x1b[1m{:<name_width$}  {:<modified_time_width$}  {:<relative_time_width$}\x1b[0m",
            "Name",
            "Modified Time",
            "Relative Time",
            name_width = max_filename_length,
            modified_time_width = modified_time_width,
            relative_time_width = relative_time_width
        );
    } else {
        println!(
            "{:<name_width$}  {:<modified_time_width$}  {:<relative_time_width$}",
            "Name",
            "Modified Time",
            "Relative Time",
            name_width = max_filename_length,
            modified_time_width = modified_time_width,
            relative_time_width = relative_time_width
        );
    }

    for file in file_info {
        let hr_time = human_readable_system_time(file.modified_time);
        let filename_abbreviated = abbreviate_filename(&file.name, max_filename_length);

        if file.file_type == FileType::Directory {
            // Print directories in blue
            if is_tty {
                println!(
                    "\x1b[34m{:<max$}\x1b[0m  {}  {}",
                    filename_abbreviated,
                    hr_time,
                    file.relative_time,
                    max = max_filename_length
                );
            } else {
                // Print directories without color
                println!(
                    "{:<max$}  {}  {}",
                    filename_abbreviated,
                    hr_time,
                    file.relative_time,
                    max = max_filename_length
                );
            }
        } else {
            // Print files normally
            println!(
                "{:<max$}  {}  {}",
                filename_abbreviated,
                hr_time,
                file.relative_time,
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
        let mut pager = pager::Pager::new();
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
