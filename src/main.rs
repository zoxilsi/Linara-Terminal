use eframe::egui;
use std::collections::{VecDeque, HashMap};
use std::process::Command;
use std::time::{Duration, Instant};
use std::env;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("Linara Terminal")
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Terminal",
        options,
        Box::new(|cc| {
            // Set up authentic terminal theme
            let mut visuals = egui::Visuals::dark();
            visuals.window_fill = egui::Color32::from_rgb(12, 12, 20);
            visuals.panel_fill = egui::Color32::from_rgb(12, 12, 20);
            visuals.extreme_bg_color = egui::Color32::from_rgb(12, 12, 20);
            cc.egui_ctx.set_visuals(visuals);
            
            Ok(Box::new(TerminalApp::new()))
        }),
    )
}

#[derive(Clone)]
struct TerminalLine {
    text: String,
    is_input: bool,
    is_prompt: bool,
}

struct TerminalApp {
    lines: VecDeque<TerminalLine>,
    input_buffer: String,
    cursor_pos: usize,
    show_cursor: bool,
    last_cursor_blink: Instant,
    command_history: Vec<String>,
    history_index: isize,
    current_dir: String,
    username: String,
    hostname: String,
    // Autocomplete fields
    autocomplete_suggestions: Vec<String>,
    autocomplete_index: isize,
    show_autocomplete: bool,
    common_commands: Vec<String>,
    command_flags: std::collections::HashMap<String, Vec<String>>,
}

impl TerminalApp {
    fn new() -> Self {
        let current_dir = env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/"))
            .to_string_lossy()
            .to_string();
        
        let username = env::var("USER").unwrap_or_else(|_| "user".to_string());
        let hostname = env::var("HOSTNAME").unwrap_or_else(|_| {
            // Try to get hostname from system
            Command::new("hostname")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|_| "localhost".to_string())
        });

        let mut app = Self {
            lines: VecDeque::new(),
            input_buffer: String::new(),
            cursor_pos: 0,
            show_cursor: true,
            last_cursor_blink: Instant::now(),
            command_history: Vec::new(),
            history_index: -1,
            current_dir,
            username,
            hostname,
            // Initialize autocomplete
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: -1,
            show_autocomplete: false,
            common_commands: vec![
                "ls".to_string(), "cd".to_string(), "pwd".to_string(), "mkdir".to_string(),
                "rm".to_string(), "cp".to_string(), "mv".to_string(), "cat".to_string(),
                "grep".to_string(), "find".to_string(), "chmod".to_string(), "ps".to_string(),
                "kill".to_string(), "tar".to_string(), "curl".to_string(), "git".to_string(),
                "clear".to_string(), "exit".to_string(), "history".to_string(), "help".to_string(),
            ],
            command_flags: HashMap::new(), // Initialize empty, will be populated below
        };

        // Initialize command flags (reduced to most common ones for speed)
        let mut command_flags = HashMap::new();
        
        // Only keep the most essential flags for speed
        command_flags.insert("ls".to_string(), vec![
            "-l".to_string(), "-a".to_string(), "-la".to_string(), "-lh".to_string(),
        ]);
        
        command_flags.insert("rm".to_string(), vec![
            "-r".to_string(), "-f".to_string(), "-rf".to_string(),
        ]);
        
        command_flags.insert("cp".to_string(), vec![
            "-r".to_string(), "-v".to_string(),
        ]);
        
        command_flags.insert("mv".to_string(), vec![
            "-v".to_string(),
        ]);
        
        command_flags.insert("grep".to_string(), vec![
            "-i".to_string(), "-r".to_string(), "-n".to_string(),
        ]);
        
        command_flags.insert("git".to_string(), vec![
            "status".to_string(), "add".to_string(), "commit".to_string(), "push".to_string(),
            "pull".to_string(),
        ]);
        
        app.command_flags = command_flags;

        // Add beautiful system information display
        app.add_system_info();
        
        // Show initial prompt
        app.show_prompt();
        
        app
    }

    fn add_line(&mut self, text: &str, is_input: bool, is_prompt: bool) {
        self.lines.push_back(TerminalLine {
            text: text.to_string(),
            is_input,
            is_prompt,
        });
        
        // Keep buffer smaller for better performance
        while self.lines.len() > 500 {
            self.lines.pop_front();
        }
    }

    fn show_prompt(&mut self) {
        let home = env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let display_dir = if self.current_dir.starts_with(&home) {
            self.current_dir.replace(&home, "~")
        } else {
            self.current_dir.clone()
        };
        
        // Shorten path to only show last 2 parent directories
        let short_path = if display_dir == "~" {
            "~".to_string()
        } else {
            let path_parts: Vec<&str> = display_dir.split('/').collect();
            if path_parts.len() <= 2 {
                display_dir.clone()
            } else {
                format!(".../{}/{}", path_parts[path_parts.len() - 2], path_parts[path_parts.len() - 1])
            }
        };
        
        // Check if we're in a Git repository and get the current branch
        let git_info = self.get_git_branch();
        
        // Create PowerShell-like header bar (without timestamp, dynamic git info)
        let header_bar = if git_info.is_empty() {
            format!("🏠 {} 📂 {}", 
                self.username, 
                short_path
            )
        } else {
            format!("🏠 {} 📂 {} {}", 
                self.username, 
                short_path,
                git_info
            )
        };
        
        // Add the header bar and simple prompt on the same line
        self.add_line(&header_bar, false, true);
    }
    
    fn add_system_info(&mut self) {
        // Add beautiful ASCII art and system information like neofetch
        self.add_line("", false, false);
        
        // Colorful ASCII Art for LINARA - Clean and readable design (left-aligned)
        self.add_line("██╗     ██╗███╗   ██╗ █████╗ ██████╗  █████╗ ", false, false);
        self.add_line("██║     ██║████╗  ██║██╔══██╗██╔══██╗██╔══██╗", false, false);
        self.add_line("██║     ██║██╔██╗ ██║███████║██████╔╝███████║", false, false);
        self.add_line("██║     ██║██║╚██╗██║██╔══██║██╔══██╗██╔══██║", false, false);
        self.add_line("███████╗██║██║ ╚████║██║  ██║██║  ██║██║  ██║", false, false);
        self.add_line("╚══════╝╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝", false, false);
        self.add_line("", false, false);
        
        // Get system information
        let username = self.username.clone();
        let hostname = self.hostname.clone();
        
        // OS Information
        let os_info = std::process::Command::new("uname")
            .args(&["-sr"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|_| "Linux".to_string());
            
        // Kernel version
        let kernel = std::process::Command::new("uname")
            .args(&["-r"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
            
        // Uptime
        let uptime = std::process::Command::new("uptime")
            .args(&["-p"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().replace("up ", ""))
            .unwrap_or_else(|_| "Unknown".to_string());
            
        // Memory info
        let memory = std::fs::read_to_string("/proc/meminfo")
            .and_then(|content| {
                let lines: Vec<&str> = content.lines().collect();
                let total_kb = lines.iter()
                    .find(|line| line.starts_with("MemTotal:"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                    
                let available_kb = lines.iter()
                    .find(|line| line.starts_with("MemAvailable:"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                    
                let used_kb = total_kb - available_kb;
                let total_gb = total_kb as f64 / 1024.0 / 1024.0;
                let used_gb = used_kb as f64 / 1024.0 / 1024.0;
                
                Ok(format!("{:.1}GB / {:.1}GB", used_gb, total_gb))
            })
            .unwrap_or_else(|_| "Unknown".to_string());
            
        // CPU info
        let cpu = std::fs::read_to_string("/proc/cpuinfo")
            .and_then(|content| {
                content.lines()
                    .find(|line| line.starts_with("model name"))
                    .and_then(|line| line.split(':').nth(1))
                    .map(|s| s.trim().to_string())
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "CPU not found"))
            })
            .unwrap_or_else(|_| "Unknown CPU".to_string());

        // Display system information in a beautiful box (left-aligned)
        self.add_line("╭─────────────────────────────────────────────────────────────╮", false, false);
        self.add_line(&format!("{}@{}", username, hostname), false, false);
        self.add_line("├─────────────────────────────────────────────────────────────┤", false, false);
        self.add_line(&format!("OS: {}", os_info), false, false);
        self.add_line(&format!("Host: {}", hostname), false, false);
        self.add_line(&format!("Kernel: {}", kernel), false, false);
        self.add_line(&format!("Uptime: {}", uptime), false, false);
        self.add_line(&format!("Terminal: Linara Terminal"), false, false);
        self.add_line(&format!("CPU: {}", cpu), false, false);
        self.add_line(&format!("Memory: {}", memory), false, false);
        self.add_line("╰─────────────────────────────────────────────────────────────╯", false, false);
    }
    
    fn get_git_branch(&self) -> String {
        // Try to get the current git branch
        let result = Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&self.current_dir)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !branch.is_empty() && branch != "HEAD" {
                    format!("⚡ {}", branch)
                } else {
                    String::new()
                }
            }
            _ => String::new()
        }
    }    fn execute_command(&mut self, command: &str) {
        if command.trim().is_empty() {
            self.show_prompt();
            
            // Clear the input buffer after command execution so new prompt is clean
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return;
        }

        // Add to history
        if !command.trim().is_empty() && (self.command_history.is_empty() || self.command_history.last() != Some(&command.to_string())) {
            self.command_history.push(command.to_string());
        }
        self.history_index = -1;

        // Command will be displayed inline with output for short commands

        let parts: Vec<String> = command.trim().split_whitespace().map(|s| s.to_string()).collect();
        if parts.is_empty() {
            self.show_prompt();
            
            // Clear the input buffer after command execution so new prompt is clean
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return;
        }

        let cmd_name = parts[0].clone();
        let args: Vec<String> = parts[1..].to_vec();

        // Check if user is asking for help
        if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
            self.format_help_output(&cmd_name);
            self.show_prompt();
            
            // Clear the input buffer after command execution so new prompt is clean
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return;
        }

        // Handle built-in commands
        match cmd_name.as_str() {
            "help" => {
                // Update the last prompt line to include the help command
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {}", last_line.text, command);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }
                
                self.add_line("🚀 Terminal Help", false, false);
                self.add_line("ls, cd, pwd, mkdir, rm, cp, mv", false, false);
                self.add_line("grep, find, cat, git, ps, kill", false, false);
                self.add_line("Type 'command --help' for details", false, false);
                self.show_prompt();
                
                // Clear the input buffer after command execution so new prompt is clean
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return;
            }
            "clear" => {
                // Update the last prompt line to include the clear command
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {}", last_line.text, command);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }
                
                self.lines.clear();
                self.show_prompt();
                
                // Clear the input buffer after command execution so new prompt is clean
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return;
            }
            "exit" => {
                // Update the last prompt line to include the exit command first
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {}", last_line.text, command);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }
                
                std::process::exit(0);
            }
            "cd" => {
                let target_dir = if args.is_empty() {
                    env::var("HOME").unwrap_or_else(|_| "/".to_string())
                } else {
                    args[0].clone()
                };
                
                let new_path = if target_dir.starts_with('/') {
                    std::path::PathBuf::from(&target_dir)
                } else {
                    std::path::PathBuf::from(&self.current_dir).join(&target_dir)
                };
                
                // Update the last prompt line to include the cd command first
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {}", last_line.text, command);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }
                
                match new_path.canonicalize() {
                    Ok(canonical_path) => {
                        if canonical_path.is_dir() {
                            self.current_dir = canonical_path.to_string_lossy().to_string();
                            let _ = env::set_current_dir(&canonical_path);
                        } else {
                            self.add_line(&format!("cd: {}: Not a directory", target_dir), false, false);
                        }
                    }
                    Err(_) => {
                        self.add_line(&format!("cd: {}: No such file or directory", target_dir), false, false);
                    }
                }
                self.show_prompt();
                
                // Clear the input buffer after command execution so new prompt is clean
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return;
            }
            "pwd" => {
                // Update the last prompt line to include the pwd command
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {}", last_line.text, command);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }
                
                let pwd = self.current_dir.clone();
                self.add_line(&pwd, false, false);
                self.show_prompt();
                
                // Clear the input buffer after command execution so new prompt is clean
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return;
            }
            "history" => {
                // Update the last prompt line to include the history command
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {}", last_line.text, command);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }
                
                let history = self.command_history.clone();
                for (i, cmd) in history.iter().enumerate() {
                    let history_line = format!(" {}: {}", i + 1, cmd);
                    self.add_line(&history_line, false, false);
                }
                self.show_prompt();
                
                // Clear the input buffer after command execution so new prompt is clean
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return;
            }
            _ => {}
        }

        // Execute external command synchronously for now
        let result = Command::new(&cmd_name)
            .args(&args)
            .current_dir(&self.current_dir)
            .output();

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                // Check if output is short enough to display inline
                let stdout_lines: Vec<&str> = stdout.lines().collect();
                let stderr_lines: Vec<&str> = stderr.lines().collect();

                let is_short_output = stdout_lines.len() <= 1 &&
                                    stderr_lines.is_empty() &&
                                    stdout.trim().len() < 80 && // Less than 80 characters
                                    !stdout.contains('\n'); // No newlines

                if is_short_output && !stdout.trim().is_empty() {
                    // Update the last prompt line to include the command and output inline
                    if let Some(last_line) = self.lines.back_mut() {
                        if last_line.is_prompt {
                            last_line.text = format!("{} > {} {}", last_line.text, command, stdout.trim());
                            last_line.is_prompt = false; // Mark as completed command
                        }
                    }
                } else {
                    // Update the last prompt line to include the command
                    if let Some(last_line) = self.lines.back_mut() {
                        if last_line.is_prompt {
                            last_line.text = format!("{} > {}", last_line.text, command);
                            last_line.is_prompt = false; // Mark as completed command
                        }
                    }

                    // Add stdout on separate lines
                    for line in stdout_lines {
                        if !line.is_empty() {
                            self.add_line(line, false, false);
                        }
                    }
                }

                // Add stderr (always on separate lines for visibility)
                for line in stderr_lines {
                    if !line.is_empty() {
                        self.add_line(&format!("ERROR: {}", line), false, false);
                    }
                }

                // Add exit status if non-zero
                if !output.status.success() {
                    if let Some(code) = output.status.code() {
                        self.add_line(&format!("Command '{}' exited with code {}", cmd_name, code), false, false);
                    }
                }
            }
            Err(e) => {
                // Update the last prompt line to include the failed command
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {} (Failed: {})", last_line.text, command, e);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }
            }
        }

        self.show_prompt();
        
        // Clear the input buffer after command execution so new prompt is clean
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    fn format_help_output(&mut self, command: &str) {
        match command {
            "ls" => {
                self.add_line("📁 ls - List files", false, false);
                self.add_line("-l (detailed), -a (hidden), -lh (sizes)", false, false);
            },
            "grep" => {
                self.add_line("🔍 grep - Search text", false, false);
                self.add_line("-i (ignore case), -r (recursive), -n (line numbers)", false, false);
            },
            "git" => {
                self.add_line("🌿 git - Version control", false, false);
                self.add_line("status, add, commit, push, pull", false, false);
            },
            _ => {
                self.add_line(&format!("ℹ️  {} - Try {} --help", command, command), false, false);
            }
        }
    }

    fn update_autocomplete(&mut self) {
        if self.input_buffer.is_empty() {
            self.show_autocomplete = false;
            self.autocomplete_suggestions.clear();
            return;
        }

        // Get the current word being typed (last word in input)
        let words: Vec<&str> = self.input_buffer.split_whitespace().collect();
        let current_word = if self.input_buffer.ends_with(' ') {
            ""
        } else {
            words.last().map_or("", |&word| word)
        };

        if current_word.is_empty() || current_word.len() < 1 { // Only start suggesting after 1 char
            self.show_autocomplete = false;
            self.autocomplete_suggestions.clear();
            return;
        }

        // Find matching suggestions
        let mut suggestions = Vec::new();
        
        // If it's the first word, match against commands
        if words.len() <= 1 {
            for cmd in &self.common_commands {
                if cmd.starts_with(current_word) && cmd != current_word {
                    suggestions.push(cmd.clone());
                    if suggestions.len() >= 5 { break; } // Limit to 5 for speed
                }
            }
        } else {
            // For subsequent words, check if we should suggest flags first
            let command = words[0];
            
            // Check if current word looks like a flag (starts with -)
            if current_word.starts_with('-') {
                // Suggest flags for this command
                if let Some(flags) = self.command_flags.get(command) {
                    for flag in flags {
                        if flag.starts_with(current_word) && flag != current_word {
                            suggestions.push(flag.clone());
                            if suggestions.len() >= 5 { break; } // Limit for speed
                        }
                    }
                }
            }
        }

        // If no command/flag suggestions found, try file/directory completion
        if suggestions.is_empty() && !current_word.is_empty() {
            if let Ok(entries) = std::fs::read_dir(&self.current_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Some(file_name) = entry.file_name().to_str() {
                            if file_name.starts_with(current_word) && file_name != current_word {
                                // Add directory indicator if it's a directory
                                let suggestion = if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                                    format!("{}/", file_name)
                                } else {
                                    file_name.to_string()
                                };
                                suggestions.push(suggestion);
                                if suggestions.len() >= 5 { break; } // Limit for speed
                            }
                        }
                    }
                }
            }
        }

        // Limit suggestions and update
        suggestions.truncate(5); // Reduced from 10 to 5 for speed
        self.autocomplete_suggestions = suggestions;
        self.show_autocomplete = !self.autocomplete_suggestions.is_empty();
        self.autocomplete_index = -1;
    }

    fn apply_autocomplete(&mut self) -> bool {
        if self.autocomplete_suggestions.is_empty() {
            return false;
        }

        // Cycle through suggestions
        if self.autocomplete_index < 0 {
            self.autocomplete_index = 0;
        } else {
            self.autocomplete_index = (self.autocomplete_index + 1) % self.autocomplete_suggestions.len() as isize;
        }

        let suggestion = &self.autocomplete_suggestions[self.autocomplete_index as usize];
        
        // Replace the current word with the suggestion
        let words: Vec<&str> = self.input_buffer.split_whitespace().collect();
        if words.is_empty() {
            self.input_buffer = suggestion.clone();
        } else {
            let mut new_buffer = words[..words.len() - 1].join(" ");
            if !new_buffer.is_empty() {
                new_buffer.push(' ');
            }
            new_buffer.push_str(suggestion);
            
            // If it's a flag or command, add a space at the end for easier continuation
            if suggestion.starts_with('-') || words.len() == 1 {
                new_buffer.push(' ');
            }
            
            self.input_buffer = new_buffer;
        }
        
        self.cursor_pos = self.input_buffer.len();
        true
    }

    fn handle_key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        match key {
            egui::Key::Enter => {
                let command = self.input_buffer.clone();
                // Input buffer will be cleared in execute_command after successful execution
                // Clear autocomplete
                self.show_autocomplete = false;
                self.autocomplete_suggestions.clear();
                self.autocomplete_index = -1;
                self.execute_command(&command);
            }
            egui::Key::Backspace => {
                if self.cursor_pos > 0 {
                    self.input_buffer.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                    self.update_autocomplete();
                }
            }
            egui::Key::Delete => {
                if self.cursor_pos < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_pos);
                    self.update_autocomplete();
                }
            }
            egui::Key::ArrowLeft => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            egui::Key::ArrowRight => {
                if self.cursor_pos < self.input_buffer.len() {
                    self.cursor_pos += 1;
                }
            }
            egui::Key::ArrowUp => {
                // Hide autocomplete when navigating history
                self.show_autocomplete = false;
                if !self.command_history.is_empty() {
                    if self.history_index < 0 {
                        self.history_index = self.command_history.len() as isize - 1;
                    } else if self.history_index > 0 {
                        self.history_index -= 1;
                    }
                    if self.history_index >= 0 {
                        self.input_buffer = self.command_history[self.history_index as usize].clone();
                        self.cursor_pos = self.input_buffer.len();
                    }
                }
            }
            egui::Key::ArrowDown => {
                // Hide autocomplete when navigating history
                self.show_autocomplete = false;
                if !self.command_history.is_empty() && self.history_index >= 0 {
                    self.history_index += 1;
                    if self.history_index >= self.command_history.len() as isize {
                        self.history_index = -1;
                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                    } else {
                        self.input_buffer = self.command_history[self.history_index as usize].clone();
                        self.cursor_pos = self.input_buffer.len();
                    }
                }
            }
            egui::Key::Home => {
                self.cursor_pos = 0;
            }
            egui::Key::End => {
                self.cursor_pos = self.input_buffer.len();
            }
            egui::Key::Tab => {
                if self.apply_autocomplete() {
                    // Tab was used for autocomplete
                } else {
                    // Fallback: add space
                    self.input_buffer.push(' ');
                    self.cursor_pos += 1;
                    self.update_autocomplete();
                }
            }
            egui::Key::Escape => {
                // Hide autocomplete suggestions
                self.show_autocomplete = false;
                self.autocomplete_suggestions.clear();
                self.autocomplete_index = -1;
            }
            _ => {
                if modifiers.ctrl {
                    match key {
                        egui::Key::C => {
                            // Ctrl+C - interrupt current command
                            self.add_line("^C", false, false);
                            self.input_buffer.clear();
                            self.cursor_pos = 0;
                            self.show_prompt();
                        }
                        egui::Key::D => {
                            // Ctrl+D - EOF/exit
                            std::process::exit(0);
                        }
                        egui::Key::L => {
                            // Ctrl+L - clear screen
                            self.lines.clear();
                            self.show_prompt();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle cursor blinking (optimized)
        if self.last_cursor_blink.elapsed() > Duration::from_millis(500) {
            self.show_cursor = !self.show_cursor;
            self.last_cursor_blink = Instant::now();
            ctx.request_repaint_after(Duration::from_millis(500)); // Only repaint when needed
        }

        // Handle keyboard input
        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        self.handle_key(*key, *modifiers);
                    }
                    egui::Event::Text(text) => {
                        for ch in text.chars() {
                            if ch.is_control() || ch == '\n' || ch == '\r' {
                                continue;
                            }
                            self.input_buffer.insert(self.cursor_pos, ch);
                            self.cursor_pos += 1;
                        }
                        self.update_autocomplete();
                    }
                    _ => {}
                }
            }
        });

        // Main terminal panel - fullscreen
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(12, 12, 20)))
            .show(ctx, |ui| {
                // Terminal content with proper margins
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(12, 12, 20))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        // Scrollable terminal area
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                                    // Display all terminal lines except the last prompt
                                    let lines_to_show: Vec<_> = if self.lines.back().map_or(false, |line| line.is_prompt) {
                                        self.lines.iter().take(self.lines.len() - 1).collect()
                                    } else {
                                        self.lines.iter().collect()
                                    };

                                    for line in lines_to_show {
                                        // Check if this is a system info line for special rendering
                                        let is_system_info = line.text.contains("██") || 
                                                            line.text.starts_with("OS:") ||
                                                            line.text.starts_with("Kernel:") ||
                                                            line.text.starts_with("Uptime:") ||
                                                            line.text.starts_with("Memory:") ||
                                                            line.text.starts_with("CPU:") ||
                                                            line.text.starts_with("Terminal:") ||
                                                            line.text.starts_with("$ ") ||
                                                            line.text.starts_with("┌─") && line.text.contains("System Information") ||
                                                            line.text.starts_with("└─");

                                        let color = if line.text.starts_with("ERROR:") {
                                            egui::Color32::from_rgb(255, 100, 100) // Red for errors
                                        } else if line.is_prompt {
                                            // Multicolor prompt styling for completed commands
                                            if line.text.starts_with("🏠") {
                                                // This will be handled by special rendering below
                                                egui::Color32::from_rgb(220, 220, 220) // Default for special case
                                            } else if line.text.starts_with("┌─") {
                                                egui::Color32::from_rgb(100, 200, 255) // Cyan for top line
                                            } else if line.text.starts_with("└─") {
                                                egui::Color32::from_rgb(255, 150, 100) // Orange for arrow
                                            } else {
                                                egui::Color32::from_rgb(100, 255, 100) // Green fallback
                                            }
                                        } else if line.is_input {
                                            egui::Color32::from_rgb(255, 255, 100) // Yellow for input
                                        } else {
                                            egui::Color32::from_rgb(220, 220, 220) // Normal text
                                        };
                                        
                                        // Special rendering for PowerShell-like header bar (completed commands)
                                        if line.text.starts_with("🏠") {
                                            // Render the colorful header bar like PowerShell for completed output
                                            ui.horizontal(|ui| {
                                                // Parse the line to extract prompt parts and any command/output
                                                let line_text = &line.text;

                                                // Split by ">" to separate prompt from command/output
                                                if let Some(prompt_end) = line_text.find(" > ") {
                                                    let prompt_part = &line_text[..prompt_end];
                                                    let command_output_part = &line_text[prompt_end + 3..]; // Skip " > "

                                                    // Parse the prompt part
                                                    let parts: Vec<&str> = prompt_part.split_whitespace().collect();

                                                    // Find the path - it's after "📂" symbol
                                                    let mut path_from_line = "~";
                                                    for (i, part) in parts.iter().enumerate() {
                                                        if *part == "📂" && i + 1 < parts.len() {
                                                            // Check if the next part is the path (not git info)
                                                            let potential_path = parts[i + 1];
                                                            if !potential_path.starts_with("⚡") {
                                                                path_from_line = potential_path;
                                                            }
                                                            break;
                                                        }
                                                    }

                                                    // Create a background frame for the header
                                                    ui.add_space(2.0);
                                                    egui::Frame::none()
                                                        .fill(egui::Color32::from_rgb(30, 30, 40))
                                                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                                        .rounding(egui::Rounding::same(6.0))
                                                        .show(ui, |ui| {
                                                            ui.horizontal(|ui| {
                                                                // Render each segment with proper colors
                                                                ui.label(
                                                                    egui::RichText::new("🏠 ")
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(100, 150, 255)) // Blue
                                                                );
                                                                ui.label(
                                                                    egui::RichText::new(&self.username)
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(255, 100, 150)) // Pink
                                                                );
                                                                ui.label(
                                                                    egui::RichText::new(" 📂 ")
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(100, 255, 150)) // Green
                                                                );
                                                                ui.label(
                                                                    egui::RichText::new(path_from_line)
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(255, 200, 100)) // Yellow
                                                                );

                                                                // Add git info if present in the prompt
                                                                for part in parts.iter() {
                                                                    if part.starts_with("⚡") {
                                                                        ui.label(
                                                                            egui::RichText::new(&format!(" {}", part))
                                                                                .font(egui::FontId::monospace(16.0))
                                                                                .color(egui::Color32::from_rgb(255, 255, 100)) // Bright yellow for git
                                                                        );
                                                                        break;
                                                                    }
                                                                }

                                                                // Add the ">" symbol
                                                                ui.label(
                                                                    egui::RichText::new(" > ")
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(150, 150, 150)) // Gray
                                                                );

                                                                // Render command/output with original terminal colors (not white)
                                                                if !command_output_part.is_empty() {
                                                                    ui.label(
                                                                        egui::RichText::new(command_output_part)
                                                                            .font(egui::FontId::monospace(16.0))
                                                                            .color(egui::Color32::from_rgb(220, 220, 220)) // Light gray like normal terminal text
                                                                    );
                                                                }
                                                            });
                                                        });
                                                } else {
                                                    // Fallback: just render as regular prompt (no command/output)
                                                    let parts: Vec<&str> = line_text.split_whitespace().collect();

                                                    // Find the path - it's after "📂" symbol
                                                    let mut path_from_line = "~";
                                                    for (i, part) in parts.iter().enumerate() {
                                                        if *part == "📂" && i + 1 < parts.len() {
                                                            let potential_path = parts[i + 1];
                                                            if !potential_path.starts_with("⚡") {
                                                                path_from_line = potential_path;
                                                            }
                                                            break;
                                                        }
                                                    }

                                                    // Create a background frame for the header
                                                    ui.add_space(2.0);
                                                    egui::Frame::none()
                                                        .fill(egui::Color32::from_rgb(30, 30, 40))
                                                        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                                        .rounding(egui::Rounding::same(6.0))
                                                        .show(ui, |ui| {
                                                            ui.horizontal(|ui| {
                                                                // Render each segment with proper colors
                                                                ui.label(
                                                                    egui::RichText::new("🏠 ")
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(100, 150, 255)) // Blue
                                                                );
                                                                ui.label(
                                                                    egui::RichText::new(&self.username)
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(255, 100, 150)) // Pink
                                                                );
                                                                ui.label(
                                                                    egui::RichText::new(" 📂 ")
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(100, 255, 150)) // Green
                                                                );
                                                                ui.label(
                                                                    egui::RichText::new(path_from_line)
                                                                        .font(egui::FontId::monospace(16.0))
                                                                        .color(egui::Color32::from_rgb(255, 200, 100)) // Yellow
                                                                );

                                                                // Add git info if present
                                                                for part in parts.iter() {
                                                                    if part.starts_with("⚡") {
                                                                        ui.label(
                                                                            egui::RichText::new(&format!(" {}", part))
                                                                                .font(egui::FontId::monospace(16.0))
                                                                                .color(egui::Color32::from_rgb(255, 255, 100)) // Bright yellow for git
                                                                        );
                                                                        break;
                                                                    }
                                                                }
                                                            });
                                                        });
                                                }
                                            });
                                        } else if line.is_prompt && line.text.starts_with("┌─") {
                                            // Render the top prompt line with multiple colors (legacy support)
                                            ui.horizontal(|ui| {
                                                let parts: Vec<&str> = line.text.split(" ").collect();
                                                for (i, part) in parts.iter().enumerate() {
                                                    let part_color = match i {
                                                        0 => egui::Color32::from_rgb(100, 200, 255), // ┌─
                                                        1 => egui::Color32::from_rgb(255, 200, 100), // 💻
                                                        2 => egui::Color32::from_rgb(150, 255, 150), // username
                                                        3 => egui::Color32::from_rgb(200, 150, 255), // ◦
                                                        4 => egui::Color32::from_rgb(255, 180, 120), // 📁
                                                        _ => egui::Color32::from_rgb(120, 255, 200), // directory
                                                    };
                                                    
                                                    ui.label(
                                                        egui::RichText::new(*part)
                                                            .font(egui::FontId::monospace(18.0))
                                                            .color(part_color)
                                                    );
                                                    if i < parts.len() - 1 {
                                                        ui.label(
                                                            egui::RichText::new(" ")
                                                                .font(egui::FontId::monospace(18.0))
                                                        );
                                                    }
                                                }
                                            });
                                        } else if is_system_info {
                                            // Special colorful rendering for system information
                                            if line.text.contains("██") {
                                                // ASCII art rendering with rainbow colors
                                                ui.horizontal(|ui| {
                                                    let chars: Vec<char> = line.text.chars().collect();
                                                    for (i, ch) in chars.iter().enumerate() {
                                                        if *ch == '█' {
                                                            // Rainbow colors for ASCII art blocks
                                                            let rainbow_colors = [
                                                                egui::Color32::from_rgb(255, 100, 100), // Red
                                                                egui::Color32::from_rgb(255, 165, 0),   // Orange
                                                                egui::Color32::from_rgb(255, 255, 0),   // Yellow
                                                                egui::Color32::from_rgb(100, 255, 100), // Green
                                                                egui::Color32::from_rgb(100, 150, 255), // Blue
                                                                egui::Color32::from_rgb(150, 100, 255), // Purple
                                                                egui::Color32::from_rgb(255, 100, 200), // Pink
                                                            ];
                                                            let color_index = (i / 2) % rainbow_colors.len();
                                                            
                                                            ui.label(
                                                                egui::RichText::new(ch.to_string())
                                                                    .font(egui::FontId::monospace(16.0))
                                                                    .color(rainbow_colors[color_index])
                                                            );
                                                        } else {
                                                            ui.label(
                                                                egui::RichText::new(ch.to_string())
                                                                    .font(egui::FontId::monospace(16.0))
                                                                    .color(egui::Color32::from_rgb(200, 200, 200))
                                                            );
                                                        }
                                                    }
                                                });
                                            } else if line.text.starts_with("OS:") {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("OS: ")
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(100, 150, 255))
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&line.text[4..])
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 255, 255))
                                                    );
                                                });
                                            } else if line.text.starts_with("Kernel:") {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("Kernel: ")
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(150, 100, 255))
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&line.text[8..])
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 255, 255))
                                                    );
                                                });
                                            } else if line.text.starts_with("Uptime:") {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("Uptime: ")
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 200, 100))
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&line.text[8..])
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 255, 255))
                                                    );
                                                });
                                            } else if line.text.starts_with("Memory:") {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("Memory: ")
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 150, 100))
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&line.text[8..])
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 255, 255))
                                                    );
                                                });
                                            } else if line.text.starts_with("CPU:") {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("CPU: ")
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 100, 255))
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&line.text[5..])
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 255, 255))
                                                    );
                                                });
                                            } else if line.text.starts_with("Terminal:") {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("Terminal: ")
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(100, 255, 255))
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&line.text[10..])
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 255, 255))
                                                    );
                                                });
                                            } else if line.text.starts_with("┌─") && line.text.contains("System Information") {
                                                ui.label(
                                                    egui::RichText::new(&line.text)
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(100, 200, 255))
                                                );
                                            } else if line.text.starts_with("└─") {
                                                ui.label(
                                                    egui::RichText::new(&line.text)
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(100, 200, 255))
                                                );
                                            } else if line.text.starts_with("$ ") {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("$ ")
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(100, 255, 150)) // Green for command prompt
                                                    );

                                                    // Render command and output with original terminal color
                                                    let text_after_dollar = &line.text[2..];
                                                    ui.label(
                                                        egui::RichText::new(text_after_dollar)
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(220, 220, 220)) // Light gray like normal terminal text
                                                    );
                                                });
                                            } else {
                                                // Fallback for other system info lines
                                                ui.label(
                                                    egui::RichText::new(&line.text)
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(150, 150, 255))
                                                );
                                            }
                                        } else {
                                            ui.label(
                                                egui::RichText::new(&line.text)
                                                    .font(egui::FontId::monospace(18.0))
                                                    .color(color)
                                            );
                                        }
                                    }

                                    // Current input line with prompt and cursor - inline style
                                    if let Some(last_line) = self.lines.back() {
                                        if last_line.is_prompt && last_line.text.starts_with("🏠") {
                                            ui.horizontal(|ui| {
                                                // Get shortened display directory
                                                let home = env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
                                                let display_dir = if self.current_dir.starts_with(&home) {
                                                    self.current_dir.replace(&home, "~")
                                                } else {
                                                    self.current_dir.clone()
                                                };
                                                
                                                let short_path = if display_dir == "~" {
                                                    "~".to_string()
                                                } else {
                                                    let path_parts: Vec<&str> = display_dir.split('/').collect();
                                                    if path_parts.len() <= 2 {
                                                        display_dir.clone()
                                                    } else {
                                                        format!(".../{}/{}", path_parts[path_parts.len() - 2], path_parts[path_parts.len() - 1])
                                                    }
                                                };
                                                
                                                // Render header segments with colors
                                                ui.label(
                                                    egui::RichText::new("🏠 ")
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(100, 150, 255)) // Blue
                                                );
                                                ui.label(
                                                    egui::RichText::new(&self.username)
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(255, 100, 150)) // Pink
                                                );
                                                ui.label(
                                                    egui::RichText::new(" 📂 ")
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(100, 255, 150)) // Green
                                                );
                                                ui.label(
                                                    egui::RichText::new(&short_path)
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(255, 200, 100)) // Yellow
                                                );
                                                
                                                // Add git info if present
                                                let git_info = self.get_git_branch();
                                                if !git_info.is_empty() {
                                                    ui.label(
                                                        egui::RichText::new(&format!(" {}", git_info))
                                                            .font(egui::FontId::monospace(16.0))
                                                            .color(egui::Color32::from_rgb(255, 255, 100)) // Bright yellow
                                                    );
                                                }
                                                
                                                // Show the prompt arrow
                                                ui.label(
                                                    egui::RichText::new(" > ")
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(100, 255, 150)) // Green prompt
                                                );

                                                // Show the input with cursor
                                                let mut display_input = self.input_buffer.clone();
                                                
                                                // Add blinking cursor
                                                if self.show_cursor {
                                                    if self.cursor_pos >= display_input.len() {
                                                        display_input.push('█');
                                                    } else {
                                                        display_input.insert(self.cursor_pos, '█');
                                                    }
                                                }

                                                ui.label(
                                                    egui::RichText::new(&display_input)
                                                        .font(egui::FontId::monospace(16.0))
                                                        .color(egui::Color32::from_rgb(255, 255, 255)) // White input text
                                                );
                                            });

                                            // Show autocomplete suggestions
                                            if self.show_autocomplete && !self.autocomplete_suggestions.is_empty() {
                                                ui.horizontal(|ui| {
                                                    ui.add_space(30.0); // Align with input area
                                                    ui.vertical(|ui| {
                                                        for (i, suggestion) in self.autocomplete_suggestions.iter().enumerate() {
                                                            let color = if i == self.autocomplete_index as usize {
                                                                egui::Color32::from_rgb(255, 255, 100) // Yellow highlight
                                                            } else {
                                                                egui::Color32::from_rgb(180, 180, 180) // Gray
                                                            };
                                                            
                                                            ui.label(
                                                                egui::RichText::new(suggestion)
                                                                    .font(egui::FontId::monospace(16.0))
                                                                    .color(color)
                                                            );
                                                        }
                                                    });
                                                });
                                            }
                                        }
                                    }
                                });
                            });

                        // Status bar (simplified)
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.small(format!("{} | Ctrl+L: clear", self.current_dir));
                        });
                    });
            });
    }
}
// Development milestone: Basic UI framework added
// Development milestone: Core terminal functionality implemented
// Development milestone: Input handling and prompt system added
// Development milestone: System info display and UI components
// Development milestone: Gemini API integration
// Development milestone: NLP and gibberish detection
// Development milestone: Auto-suggestions implemented
