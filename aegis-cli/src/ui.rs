use colored::*;
use indicatif::{ProgressBar, ProgressStyle};

/// Create a progress bar with the specified length
pub fn progress_bar(len: u64) -> ProgressBar {
    ProgressBar::new(len)
}

/// Print a success message with green checkmark
pub fn success(message: &str) {
    println!("{} {}", "✓".bright_green().bold(), message.bright_green());
}

/// Print a warning message with yellow warning icon
pub fn warning(message: &str) {
    println!("{} {}", "⚠".bright_yellow().bold(), message.yellow());
}

/// Print an error message with red X
pub fn error(message: &str) {
    println!("{} {}", "✗".red().bold(), message.red());
}

/// Print an info message with blue info icon
pub fn info(message: &str) {
    println!("{} {}", "ℹ".bright_blue().bold(), message);
}

/// Print a step/process message
pub fn step(step: usize, total: usize, message: &str) {
    println!("{} {}", format!("[{}/{}]", step, total).bright_cyan().bold(), message);
}

/// Print a header with decorative formatting
pub fn header(title: &str) {
    println!();
    println!("{}", "═".repeat(title.len() + 4).bright_blue());
    println!("{} {} {}", "══".bright_blue(), title.bright_white().bold(), "══".bright_blue());
    println!("{}", "═".repeat(title.len() + 4).bright_blue());
    println!();
}

/// Print a section divider
pub fn divider() {
    println!("{}", "─".repeat(60).dimmed());
}

/// Format file size in human-readable format
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Format duration in human-readable format
pub fn format_duration(duration_ms: u64) -> String {
    let seconds = duration_ms / 1000;
    let ms = duration_ms % 1000;
    
    if seconds >= 60 {
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        format!("{}m {}s", minutes, remaining_seconds)
    } else if seconds > 0 {
        format!("{}.{:03}s", seconds, ms)
    } else {
        format!("{}ms", ms)
    }
}

/// Create a table-style output for key-value pairs
pub fn print_table(title: &str, items: &[(String, String)]) {
    if !title.is_empty() {
        println!("{}", title.bright_blue().bold());
    }
    
    let max_key_width = items.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    
    for (key, value) in items {
        println!("  {:<width$} {}", 
            key.dimmed(), 
            value,
            width = max_key_width
        );
    }
}

/// Confirmation prompt
pub fn confirm(message: &str, default: bool) -> bool {
    use std::io;

    let prompt = if default {
        format!("{} [Y/n]: ", message)
    } else {
        format!("{} [y/N]: ", message)
    };

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();

    confirm_with_reader(&mut reader, &mut stdout, &prompt, default, warning)
}

fn confirm_with_reader<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
    default: bool,
    mut warn_fn: F,
) -> bool
where
    R: std::io::BufRead,
    W: std::io::Write,
    F: FnMut(&str),
{
    use std::io::Write as _;

    let mut input = String::new();

    loop {
        write!(writer, "{}", prompt.bright_yellow()).unwrap();
        writer.flush().unwrap();

        input.clear();

        match reader.read_line(&mut input) {
            Ok(0) => return default,
            Ok(_) => {
                let trimmed = input.trim();

                if trimmed.is_empty() {
                    return default;
                }

                let normalized = trimmed.to_ascii_lowercase();
                match normalized.as_str() {
                    "y" | "yes" => return true,
                    "n" | "no" => return false,
                    _ => warn_fn("Invalid response. Please enter y/n."),
                }
            }
            Err(err) => panic!("Failed to read input: {}", err),
        }
    }
}

/// Multiple choice selection
pub fn select(message: &str, choices: &[String]) -> Option<usize> {
    use std::io::{self, Write};
    
    println!("{}", message.bright_blue().bold());
    
    for (i, choice) in choices.iter().enumerate() {
        println!("  {}: {}", i + 1, choice);
    }
    
    print!("\nSelect option (1-{}): ", choices.len());
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    match input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= choices.len() => Some(n - 1),
        _ => {
            warning("Invalid selection.");
            None
        }
    }
}

/// Progress bar styles
pub mod progress_styles {
    use indicatif::ProgressStyle;
    
    pub fn default_bar() -> ProgressStyle {
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-")
    }
    
    pub fn spinner() -> ProgressStyle {
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    }
    
    pub fn percentage() -> ProgressStyle {
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% ({eta})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
        assert_eq!(format_file_size(1073741824), "1.0 GB");
    }
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0ms");
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1500), "1.500s");
        assert_eq!(format_duration(65000), "1m 5s");
        assert_eq!(format_duration(125000), "2m 5s");
    }

    #[test]
    fn confirm_handles_multiple_invalid_inputs_before_yes() {
        use std::io::Cursor;

        let input = b"maybe\nwhat\nYes\n";
        let mut reader = Cursor::new(&input[..]);
        let mut output = Vec::new();
        let mut warnings = Vec::new();

        let result = {
            let warnings_ref = &mut warnings;
            confirm_with_reader(
                &mut reader,
                &mut output,
                "Proceed? [y/N]: ",
                false,
                |msg| warnings_ref.push(msg.to_string()),
            )
        };

        assert!(result);
        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .all(|msg| msg == "Invalid response. Please enter y/n."));
    }

    #[test]
    fn confirm_returns_default_after_multiple_invalid_inputs_and_eof() {
        use std::io::Cursor;

        let input = b"???\nnope";
        let mut reader = Cursor::new(&input[..]);
        let mut output = Vec::new();
        let mut warnings = Vec::new();

        let result = {
            let warnings_ref = &mut warnings;
            confirm_with_reader(
                &mut reader,
                &mut output,
                "Proceed? [Y/n]: ",
                true,
                |msg| warnings_ref.push(msg.to_string()),
            )
        };

        assert!(result);
        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .all(|msg| msg == "Invalid response. Please enter y/n."));
    }
}
