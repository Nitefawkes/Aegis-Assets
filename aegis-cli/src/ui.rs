use colored::*;
use indicatif::ProgressBar;

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
    use std::io::{self, Write};
    
    let prompt = if default {
        format!("{} [Y/n]: ", message)
    } else {
        format!("{} [y/N]: ", message)
    };
    
    print!("{}", prompt.bright_yellow());
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => {
            warning("Invalid response. Please enter y/n.");
            confirm(message, default)
        }
    }
}

/// Multiple choice selection
pub fn select(message: &str, choices: &[String]) -> Option<usize> {
    use std::io;

    let mut stdout = io::stdout();
    let mut stdin = io::stdin().lock();

    select_with_io(message, choices, &mut stdin, &mut stdout)
}

fn select_with_io<R, W>(
    message: &str,
    choices: &[String],
    reader: &mut R,
    writer: &mut W,
) -> Option<usize>
where
    R: std::io::BufRead,
    W: std::io::Write,
{
    writeln!(writer, "{}", message.bright_blue().bold()).unwrap();

    for (i, choice) in choices.iter().enumerate() {
        writeln!(writer, "  {}: {}", i + 1, choice).unwrap();
    }

    let mut input = String::new();

    loop {
        write!(writer, "\nSelect option (1-{}): ", choices.len()).unwrap();
        writer.flush().unwrap();
        input.clear();

        match reader.read_line(&mut input) {
            Ok(0) => return None,
            Ok(_) => {
                let trimmed = input.trim();

                if trimmed.is_empty() {
                    return None;
                }

                match trimmed.parse::<usize>() {
                    Ok(n) if n > 0 && n <= choices.len() => return Some(n - 1),
                    _ => warning("Invalid selection."),
                }
            }
            Err(err) => panic!("Failed to read input: {}", err),
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
    fn test_select_reprompts_until_valid_input() {
        use std::io::Cursor;

        let choices = vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ];
        let mut input = Cursor::new(b"0\n4\n2\n".to_vec());
        let mut output = Vec::new();

        let selection =
            super::select_with_io("Choose an option", &choices, &mut input, &mut output);

        assert_eq!(selection, Some(1));
    }

    #[test]
    fn test_select_returns_none_on_empty_input() {
        use std::io::Cursor;

        let choices = vec!["Only".to_string()];
        let mut input = Cursor::new(b"\n".to_vec());
        let mut output = Vec::new();

        let selection = super::select_with_io("Choose", &choices, &mut input, &mut output);

        assert_eq!(selection, None);
    }
}
