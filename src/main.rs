use anyhow::{Context, Result};
use atty::Stream;
use chrono::{DateTime, Local, Utc};
use clap::Parser;
use clio::ClioPath;
use expanduser::expanduser;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use terminal_size::{terminal_size, Height, Width};
use timediff::TimeDiff;
use unicode_segmentation::UnicodeSegmentation;

const DEFAULT_WIDTH: usize = 80;
const DEFAULT_HEIGHT: u16 = 24;

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

    /// Show hidden files
    #[clap(short, long)]
    show_hidden: bool,
}

#[derive(PartialEq, Eq, Debug)]
enum FileType {
    File,
    Directory,
    Symlink,
    Hidden,
    Dotfile,
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
    // need to create a duration negative to now
    let duration = format!("-{}s", duration.as_secs());
    Ok(TimeDiff::to_diff(duration).parse()?)
}

fn human_readable_system_time(t: SystemTime) -> String {
    let datetime: DateTime<Utc> = DateTime::<Utc>::from(t);
    let local_time: DateTime<Local> = datetime.with_timezone(&Local);
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
fn list_dir(path: &Path, num_files: &usize, show_hidden: bool) -> Result<()> {
    let path_str = path.to_str().expect("Unable to convert path to string");
    let path = expanduser(path_str)?;
    let raw_entries = path.read_dir().expect("Failed to read directory");
    let entries = raw_entries.filter_map(|entry| entry.ok());
    let mut file_info: Vec<File> = entries
        .filter_map(|entry| get_path_mtime(entry).ok())
        .collect();
    // sort by modified time and truncate to the requested number of files

    let allowed_types = if show_hidden {
        vec![
            FileType::File,
            FileType::Directory,
            FileType::Symlink,
            FileType::Hidden,
            FileType::Dotfile,
        ]
    } else {
        vec![FileType::File, FileType::Directory]
    };
    file_info.sort_by_key(|f| f.modified_time);
    file_info.reverse();
    file_info.retain(|f| {
        allowed_types.contains(&f.file_type) && (!f.name.starts_with('.') || show_hidden)
    });
    file_info.truncate(*num_files);
    print_file_info(file_info)?;
    Ok(())
}

fn get_path_mtime(entry: fs::DirEntry) -> Result<File> {
    let path = entry.path();
    let metadata = fs::metadata(&path);
    // we we can't get metadata, return an Error
    let metadata = metadata.context("Unable to get metadata")?;
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
    let is_tty = atty::is(Stream::Stdout);

    // Get terminal width or use default if not available
    let term_width = if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        DEFAULT_WIDTH
    };

    let total_spacing = 4;
    let available_width = term_width.saturating_sub(total_spacing);

    let name_width = (available_width * 5) / 10;
    let modified_time_width = (available_width * 3) / 10;
    let relative_time_width = available_width - name_width - modified_time_width;

    let name_width = name_width.max(20);
    let modified_time_width = modified_time_width.max(15);
    let relative_time_width = relative_time_width.max(10);

    // Show table headers
    if is_tty {
        println!(
            "\x1b[1m{:<name_width$}  {:<modified_time_width$}  {:<relative_time_width$}\x1b[0m",
            "Name",
            "Modified Time",
            "Relative Time",
            name_width = name_width,
            modified_time_width = modified_time_width,
            relative_time_width = relative_time_width
        );
    } else {
        println!(
            "{:<name_width$}  {:<modified_time_width$}  {:<relative_time_width$}",
            "Name",
            "Modified Time",
            "Relative Time",
            name_width = name_width,
            modified_time_width = modified_time_width,
            relative_time_width = relative_time_width
        );
    }

    for file in file_info {
        let hr_time = human_readable_system_time(file.modified_time);
        let filename_abbreviated = abbreviate_filename(&file.name, name_width);
        if file.file_type == FileType::Directory {
            if is_tty {
                println!(
                    "\x1b[34m{:<width$}\x1b[0m  {:<modified_width$}  {:<relative_width$}",
                    filename_abbreviated,
                    hr_time,
                    file.relative_time,
                    width = name_width,
                    modified_width = modified_time_width,
                    relative_width = relative_time_width
                );
            } else {
                println!(
                    "{:<width$}  {:<modified_width$}  {:<relative_width$}",
                    filename_abbreviated,
                    hr_time,
                    file.relative_time,
                    width = name_width,
                    modified_width = modified_time_width,
                    relative_width = relative_time_width
                );
            }
        } else {
            println!(
                "{:<width$}  {:<modified_width$}  {:<relative_width$}",
                filename_abbreviated,
                hr_time,
                file.relative_time,
                width = name_width,
                modified_width = modified_time_width,
                relative_width = relative_time_width
            );
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    let term_height = if let Some((_, Height(h))) = terminal_size() {
        h
    } else {
        DEFAULT_HEIGHT
    };
    // use a pager if the number of files exceeds the terminal height
    let display_height = opts.num_files - 1;
    if display_height > term_height as usize {
        let mut pager = pager::Pager::new();
        pager.setup();
    }

    let res = list_dir(&opts.directory, &opts.num_files, opts.show_hidden);
    match res {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
