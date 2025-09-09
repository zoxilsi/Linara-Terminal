use eframe::egui;
use std::collections::{VecDeque, HashMap};
use std::process::Command;
use std::time::{Duration, Instant};
use std::env;
use std::os::unix::fs::PermissionsExt;
use crate::ai_assistant::AIAssistant;

pub mod ai_assistant;

fn main() -> Result<(), eframe::Error> {
    // Load .env if present
    let _ = dotenvy::dotenv();
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
    // Clipboard and selection support
    selection_start: Option<usize>,
    selection_end: Option<usize>,
    pending_copy: Option<String>,
    pending_paste: bool,
    clipboard_content: String,
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
    path_commands: Vec<String>,
    command_flags: std::collections::HashMap<String, Vec<String>>,
    // Enhanced suggestion system
    command_cache: HashMap<String, Vec<String>>, // Cache for different contexts
    last_path_scan: Instant,
    fuzzy_enabled: bool,
    // AI
    ai: AIAssistant,
    rt: tokio::runtime::Runtime,
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
            // Initialize clipboard and selection
            selection_start: None,
            selection_end: None,
            pending_copy: None,
            pending_paste: false,
            clipboard_content: String::new(),
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
                // File operations
                "ls".to_string(), "cd".to_string(), "pwd".to_string(), "mkdir".to_string(),
                "rm".to_string(), "cp".to_string(), "mv".to_string(), "cat".to_string(),
                "less".to_string(), "more".to_string(), "head".to_string(), "tail".to_string(),
                "touch".to_string(), "chmod".to_string(), "chown".to_string(), "ln".to_string(),
                "find".to_string(), "locate".to_string(), "which".to_string(), "whereis".to_string(),

                // Text processing
                "grep".to_string(), "sed".to_string(), "awk".to_string(), "cut".to_string(),
                "sort".to_string(), "uniq".to_string(), "wc".to_string(), "diff".to_string(),
                "patch".to_string(), "tr".to_string(), "fmt".to_string(), "fold".to_string(),

                // System info
                "ps".to_string(), "top".to_string(), "htop".to_string(), "df".to_string(),
                "du".to_string(), "free".to_string(), "uptime".to_string(), "who".to_string(),
                "w".to_string(), "id".to_string(), "uname".to_string(), "hostname".to_string(),

                // Process management
                "kill".to_string(), "killall".to_string(), "pkill".to_string(), "pgrep".to_string(),
                "nice".to_string(), "renice".to_string(), "nohup".to_string(), "jobs".to_string(),
                "bg".to_string(), "fg".to_string(),

                // Archive operations
                "tar".to_string(), "gzip".to_string(), "gunzip".to_string(), "bzip2".to_string(),
                "bunzip2".to_string(), "xz".to_string(), "unxz".to_string(), "zip".to_string(),
                "unzip".to_string(), "rar".to_string(), "unrar".to_string(),

                // Network
                "ping".to_string(), "traceroute".to_string(), "dig".to_string(), "nslookup".to_string(),
                "curl".to_string(), "wget".to_string(), "ssh".to_string(), "scp".to_string(),
                "rsync".to_string(), "ftp".to_string(), "sftp".to_string(), "telnet".to_string(),
                "netstat".to_string(), "ss".to_string(), "ip".to_string(), "ifconfig".to_string(),

                // Development tools
                "git".to_string(), "make".to_string(), "gcc".to_string(), "g++".to_string(),
                "python".to_string(), "python3".to_string(), "pip".to_string(), "pip3".to_string(),
                "node".to_string(), "npm".to_string(), "yarn".to_string(), "cargo".to_string(),
                "rustc".to_string(), "java".to_string(), "javac".to_string(), "gradle".to_string(),
                "maven".to_string(), "docker".to_string(), "docker-compose".to_string(),

                // Package management
                "apt".to_string(), "apt-get".to_string(), "dpkg".to_string(), "snap".to_string(),
                "flatpak".to_string(), "pacman".to_string(), "yum".to_string(), "dnf".to_string(),
                "zypper".to_string(), "brew".to_string(),

                // System administration
                "sudo".to_string(), "su".to_string(), "passwd".to_string(), "useradd".to_string(),
                "usermod".to_string(), "userdel".to_string(), "groupadd".to_string(), "groupmod".to_string(),
                "systemctl".to_string(), "service".to_string(), "journalctl".to_string(),
                "crontab".to_string(), "at".to_string(), "mount".to_string(), "umount".to_string(),
                "fdisk".to_string(), "mkfs".to_string(), "fsck".to_string(), "dd".to_string(),

                // Shell builtins and utilities
                "echo".to_string(), "printf".to_string(), "read".to_string(), "test".to_string(),
                "expr".to_string(), "bc".to_string(), "date".to_string(), "cal".to_string(),
                "sleep".to_string(), "time".to_string(), "watch".to_string(), "timeout".to_string(),
                "xargs".to_string(), "tee".to_string(), "yes".to_string(), "seq".to_string(),

                // Terminal utilities
                "clear".to_string(), "reset".to_string(), "tput".to_string(), "stty".to_string(),
                "screen".to_string(), "tmux".to_string(), "history".to_string(), "alias".to_string(),
                "export".to_string(), "unset".to_string(), "source".to_string(), "exit".to_string(),
                "logout".to_string(), "shutdown".to_string(), "reboot".to_string(), "halt".to_string(),

                // File system utilities
                "stat".to_string(), "file".to_string(), "basename".to_string(), "dirname".to_string(),
                "realpath".to_string(), "readlink".to_string(), "mktemp".to_string(), "tempfile".to_string(),
                "split".to_string(), "csplit".to_string(), "comm".to_string(), "join".to_string(),
                "paste".to_string(), "expand".to_string(), "unexpand".to_string(),

                // Development and debugging
                "strace".to_string(), "ltrace".to_string(), "gdb".to_string(), "valgrind".to_string(),
                "perf".to_string(), "dmesg".to_string(), "syslog".to_string(), "logger".to_string(),
                "lsof".to_string(), "fuser".to_string(), "vmstat".to_string(), "iostat".to_string(),
                "sar".to_string(), "mpstat".to_string(),
            ],
            path_commands: Vec::new(),
            command_flags: HashMap::new(), // Initialize empty, will be populated below
            // Enhanced suggestion system
            command_cache: HashMap::new(),
            last_path_scan: Instant::now(),
            fuzzy_enabled: true,
            ai: AIAssistant::new(),
            rt: tokio::runtime::Runtime::new().expect("tokio runtime"),
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

        // Scan PATH for available commands
        app.scan_path_commands();

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
                self.add_line("Type 'explain <command>' for simple explanations", false, false);
                self.add_line("Type 'what is <command>' for simple explanations", false, false);
                self.add_line("Type 'command --help' for details", false, false);
                self.show_prompt();
                
                // Clear the input buffer after command execution so new prompt is clean
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return;
            }
            "explain" | "whatis" => {
                // Update the last prompt line to include the explain command
                if let Some(last_line) = self.lines.back_mut() {
                    if last_line.is_prompt {
                        last_line.text = format!("{} > {}", last_line.text, command);
                        last_line.is_prompt = false; // Mark as completed command
                    }
                }

                if args.is_empty() {
                    self.add_line("Usage: explain <command>", false, false);
                    self.add_line("Example: explain ls", false, false);
                } else {
                    let cmd_to_explain = &args[0];
                    self.explain_command(cmd_to_explain);
                }
                self.show_prompt();

                // Clear the input buffer after command execution so new prompt is clean
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return;
            }
            "what" => {
                // Handle "what is <command>" syntax
                if args.len() >= 2 && args[0] == "is" {
                    // Update the last prompt line to include the what is command
                    if let Some(last_line) = self.lines.back_mut() {
                        if last_line.is_prompt {
                            last_line.text = format!("{} > {}", last_line.text, command);
                            last_line.is_prompt = false; // Mark as completed command
                        }
                    }

                    let cmd_to_explain = &args[1];
                    self.explain_command(cmd_to_explain);
                    self.show_prompt();

                    // Clear the input buffer after command execution so new prompt is clean
                    self.input_buffer.clear();
                    self.cursor_pos = 0;
                    return;
                } else {
                    // Fall through to external command execution
                }
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
                // Try AI interpretation only when command/binary not found
                let err_msg = format!("{}", e);
                let is_cmd_missing = err_msg.contains("No such file or directory") || err_msg.contains("command not found");

                if is_cmd_missing {
                    // Check for instant commands first (ultra-fast, no AI call)
                    if let Some(instant_cmd) = AIAssistant::get_instant_command(command) {
                        // Update the last prompt line to include the command
                        if let Some(last_line) = self.lines.back_mut() {
                            if last_line.is_prompt {
                                last_line.text = format!("{} > {}", last_line.text, command);
                                last_line.is_prompt = false; // Mark as completed command
                            }
                        }
                        self.add_line(&format!("⚡ {}", &instant_cmd), false, false);
                        self.run_command_and_render(&instant_cmd);
                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                        return;
                    }

                    // Close the current prompt line with the raw input
                    if let Some(last_line) = self.lines.back_mut() {
                        if last_line.is_prompt {
                            last_line.text = format!("{} > {}", last_line.text, command);
                            last_line.is_prompt = false;
                        }
                    }
                    self.add_line("⚡ Processing...", false, false);
                    // Run AI generation without borrowing &mut self across await
                    let input_clone = command.to_string();
                    let ai_result = self.rt.block_on(self.ai.generate_command(&input_clone));
                    match ai_result {
                        Ok(cmd) => {
                            self.add_line(&format!("✅ {}", &cmd), false, false);
                            self.run_command_and_render(&cmd);
                            // self.show_prompt(); // Removed to avoid duplicate
                            self.input_buffer.clear();
                            self.cursor_pos = 0;
                        }
                        Err(err) => {
                            let msg = err.to_string();
                            if msg.contains("I_DONT_UNDERSTAND") || msg.contains("don't understand") {
                                self.add_line("🤔 I don't understand that request. Please try:", false, false);
                                self.add_line("   • Use clear commands like 'list files', 'create folder test'", false, false);
                                self.add_line("   • Avoid gibberish or random characters", false, false);
                                self.add_line("   • Try rephrasing your request", false, false);
                            } else if msg.contains("deadline has elapsed") {
                                self.add_line("⏰ AI timed out. Try again.", false, false);
                            } else {
                                self.add_line(&format!("❌ Could not interpret: {}", command), false, false);
                                self.add_line(&format!("   (AI error: {})", msg), false, false);
                            }
                            // self.show_prompt(); // Removed to avoid duplicate
                            self.input_buffer.clear();
                            self.cursor_pos = 0;
                        }
                    }
                } else {
                    // Update the last prompt line to include the failed command
                    if let Some(last_line) = self.lines.back_mut() {
                        if last_line.is_prompt {
                            last_line.text = format!("{} > {} (Failed: {})", last_line.text, command, e);
                            last_line.is_prompt = false; // Mark as completed command
                        }
                    }
                }
            }
        }

    self.show_prompt();
        
        // Clear the input buffer after command execution so new prompt is clean
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    fn run_command_and_render(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        if parts.is_empty() {
            self.add_line("❌ Empty command", false, false);
            return;
        }
        let (name, args) = (parts[0], &parts[1..]);

        // Special handling for cd
        if name == "cd" {
            let target_dir = if args.is_empty() {
                env::var("HOME").unwrap_or_else(|_| "/".to_string())
            } else {
                args[0].to_string()
            };
            
            let new_path = if target_dir.starts_with('/') {
                std::path::PathBuf::from(&target_dir)
            } else {
                std::path::PathBuf::from(&self.current_dir).join(&target_dir)
            };
            
            match new_path.canonicalize() {
                Ok(canonical_path) => {
                    if canonical_path.is_dir() {
                        self.current_dir = canonical_path.to_string_lossy().to_string();
                        let _ = env::set_current_dir(&canonical_path);
                        self.add_line("✅ Directory changed", false, false);
                    } else {
                        self.add_line(&format!("cd: {}: Not a directory", target_dir), false, false);
                    }
                }
                Err(_) => {
                    self.add_line(&format!("cd: {}: No such file or directory", target_dir), false, false);
                }
            }
            return;
        }

        let output = Command::new(name).args(args).current_dir(&self.current_dir).output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                for line in stdout.lines() { if !line.is_empty() { self.add_line(line, false, false); } }
                for line in stderr.lines() { if !line.is_empty() { self.add_line(&format!("ERROR: {}", line), false, false); } }
                if out.status.success() && stdout.trim().is_empty() && stderr.trim().is_empty() {
                    self.add_line("✅ Command executed successfully", false, false);
                }
            }
            Err(e) => {
                self.add_line(&format!("❌ Failed to execute '{}': {}", name, e), false, false);
            }
        }
    }

    async fn execute_natural_language(&mut self, natural_input: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Ask AI to convert NL to a command
        let cmd = match self.ai.generate_command(natural_input).await {
            Ok(c) => c,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("I_DONT_UNDERSTAND") || msg.contains("don't understand") {
                    self.add_line("🤔 I don't understand that request. Please try:", false, false);
                    self.add_line("   • Use clear commands like 'list files', 'create folder test'", false, false);
                    self.add_line("   • Avoid gibberish or random characters", false, false);
                    self.add_line("   • Try rephrasing your request", false, false);
                } else if msg.contains("deadline has elapsed") {
                    self.add_line("⏰ AI timed out. Try again.", false, false);
                } else {
                    self.add_line(&format!("❌ AI Error: {}", msg), false, false);
                }
                self.show_prompt();
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return Ok(false);
            }
        };

        self.add_line(&format!("🔧 AI suggests: {}", &cmd), false, false);
        // Execute suggested command
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        if parts.is_empty() {
            self.add_line("❌ AI returned empty command", false, false);
            self.show_prompt();
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return Ok(false);
        }
        let (name, args) = (parts[0], &parts[1..]);
        let output = Command::new(name).args(args).current_dir(&self.current_dir).output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                for line in stdout.lines() { if !line.is_empty() { self.add_line(line, false, false); } }
                for line in stderr.lines() { if !line.is_empty() { self.add_line(&format!("ERROR: {}", line), false, false); } }
                if out.status.success() && stdout.trim().is_empty() && stderr.trim().is_empty() {
                    self.add_line("✅ Command executed successfully", false, false);
                }
            }
            Err(e) => {
                self.add_line(&format!("❌ Failed to execute '{}': {}", name, e), false, false);
            }
        }
        self.show_prompt();
        self.input_buffer.clear();
        self.cursor_pos = 0;
        Ok(true)
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

    fn explain_command(&mut self, cmd: &str) {
        let explanation = match cmd {
            "ls" => {
                "📁 ls - List files and directories\n  -l  : Long format (permissions, size, date)\n  -a  : Show hidden files (start with .)\n  -h  : Human readable sizes\n  -la : Show all files in long format"
            }
            "cd" => {
                "📂 cd - Change directory\n  Usage: cd <directory>\n  cd ..    : Go up one level\n  cd ~     : Go to home directory\n  cd /     : Go to root directory"
            }
            "pwd" => {
                "📍 pwd - Print working directory\n  Shows your current location in the file system\n  No flags needed - just type 'pwd'"
            }
            "mkdir" => {
                "📁 mkdir - Make directory\n  -p  : Create parent directories if needed\n  Usage: mkdir <dirname> or mkdir -p path/to/dir"
            }
            "rm" => {
                "🗑️ rm - Remove files/directories\n  -r  : Remove directories recursively\n  -f  : Force (no confirmation)\n  -rf : Force remove directory and contents"
            }
            "cp" => {
                "📋 cp - Copy files/directories\n  -r  : Copy directories recursively\n  -v  : Verbose (show what it's doing)\n  Usage: cp <source> <destination>"
            }
            "mv" => {
                "📦 mv - Move/rename files\n  Usage: mv <old_name> <new_name>\n  Can move files between directories\n  Same command for renaming and moving"
            }
            "cat" => {
                "📄 cat - Show file contents\n  -n  : Show line numbers\n  Usage: cat <filename>\n  Concatenates and displays files"
            }
            "grep" => {
                "🔍 grep - Search for text patterns\n  -i  : Case insensitive\n  -n  : Show line numbers\n  -r  : Search recursively\n  Usage: grep 'pattern' <file>"
            }
            "find" => {
                "🔎 find - Search for files\n  -name : Search by filename\n  -type : Search by type (f=file, d=dir)\n  Usage: find . -name '*.txt'"
            }
            "ps" => {
                "📊 ps - Show running processes\n  -a  : All processes\n  -u  : Show user info\n  -x  : Include processes without terminal\n  aux : Show all processes with details"
            }
            "kill" => {
                "💀 kill - Stop processes\n  -9  : Force kill (SIGKILL)\n  Usage: kill <PID> or kill -9 <PID>\n  Use 'ps' to find process IDs"
            }
            "top" | "htop" => {
                "📈 top - Monitor system processes\n  Shows CPU, memory usage\n  Press 'q' to quit\n  htop is a nicer version if installed"
            }
            "df" => {
                "💾 df - Show disk space usage\n  -h  : Human readable sizes\n  Shows space used/free on all disks"
            }
            "du" => {
                "📏 du - Show directory/file sizes\n  -h  : Human readable\n  -s  : Summary only\n  Usage: du -sh <directory>"
            }
            "chmod" => {
                "🔐 chmod - Change file permissions\n  +x  : Make executable\n  755 : Owner full, others read/execute\n  Usage: chmod +x <file> or chmod 755 <file>"
            }
            "chown" => {
                "👤 chown - Change file owner\n  Usage: chown <user> <file>\n  chown <user>:<group> <file>\n  Usually needs sudo"
            }
            "tar" => {
                "📦 tar - Archive files\n  -c  : Create archive\n  -x  : Extract archive\n  -f  : Specify filename\n  -z  : Use gzip compression\n  -v  : Verbose\n  Examples:\n    tar -czf archive.tar.gz files/\n    tar -xzf archive.tar.gz"
            }
            "wget" | "curl" => {
                "🌐 wget/curl - Download from internet\n  wget <URL>  : Download file\n  curl -O <URL> : Download file\n  curl <URL>   : Show webpage content"
            }
            "ssh" => {
                "🔗 ssh - Connect to remote server\n  Usage: ssh user@hostname\n  -i <key> : Use specific SSH key\n  -p <port> : Use different port"
            }
            "git" => {
                "📚 git - Version control\n  status    : Show current state\n  add .     : Stage all changes\n  commit -m 'msg' : Save changes\n  push      : Upload to remote\n  pull      : Download from remote\n  clone <URL> : Copy repository"
            }
            "apt" | "yum" | "dnf" | "pacman" => {
                "📦 Package managers\n  apt install <pkg>   : Install package\n  apt remove <pkg>    : Remove package\n  apt search <pkg>    : Search packages\n  apt update          : Update package list\n  apt upgrade         : Upgrade all packages"
            }
            "systemctl" => {
                "⚙️ systemctl - Control system services\n  start <service>   : Start service\n  stop <service>    : Stop service\n  status <service>  : Show service status\n  enable <service>  : Start on boot\n  restart <service> : Restart service"
            }
            "ping" => {
                "📡 ping - Test network connection\n  -c 4  : Send 4 packets only\n  Usage: ping <hostname or IP>\n  Tests if a host is reachable"
            }
            "ifconfig" | "ip" => {
                "🌐 Network configuration\n  ifconfig          : Show network interfaces\n  ip addr show      : Show IP addresses\n  ip route show     : Show routing table"
            }
            "man" => {
                "📖 man - Manual pages\n  Usage: man <command>\n  Shows detailed help for commands\n  Press 'q' to quit, '/' to search"
            }
            "history" => {
                "📜 history - Command history\n  Shows previously typed commands\n  !123 : Run command number 123\n  !!   : Run last command"
            }
            "alias" => {
                "🏷️ alias - Create command shortcuts\n  alias ll='ls -la'  : Create shortcut\n  alias              : Show all aliases\n  unalias <name>     : Remove alias"
            }
            "echo" => {
                "🔊 echo - Print text\n  -n  : No newline at end\n  Usage: echo 'Hello World'\n  echo $HOME : Show environment variable"
            }
            "which" => {
                "🔍 which - Find where a command is located\n  Usage: which <command>\n  Shows the full path to executable"
            }
            "whoami" => {
                "👤 whoami - Show current user\n  Shows your username\n  Same as 'id -un'"
            }
            "date" => {
                "📅 date - Show current date/time\n  +'%Y-%m-%d' : Custom format\n  Shows system date and time"
            }
            "cal" => {
                "📅 cal - Show calendar\n  cal           : Current month\n  cal 2024      : Specific year\n  cal 12 2024   : Specific month/year"
            }
            "head" | "tail" => {
                "📄 head/tail - Show file beginning/end\n  -n 10  : Show 10 lines\n  -f     : Follow (tail only, for logs)\n  Usage: head -n 5 <file> or tail -f <logfile>"
            }
            "sort" => {
                "🔤 sort - Sort lines in file\n  -n  : Numeric sort\n  -r  : Reverse order\n  -u  : Unique lines only\n  Usage: sort <file> or command | sort"
            }
            "wc" => {
                "📊 wc - Count lines/words/characters\n  -l  : Count lines only\n  -w  : Count words only\n  -c  : Count characters only\n  Usage: wc <file> or command | wc -l"
            }
            "diff" => {
                "🔄 diff - Compare files\n  -u  : Unified format\n  Usage: diff file1 file2\n  Shows differences between files"
            }
            "mount" | "umount" => {
                "💿 mount - Mount/unmount filesystems\n  mount /dev/sdb1 /mnt  : Mount device\n  umount /mnt           : Unmount\n  Usually needs sudo"
            }
            "free" => {
                "🧠 free - Show memory usage\n  -h  : Human readable\n  Shows RAM and swap usage"
            }
            "uname" => {
                "💻 uname - Show system information\n  -a  : All information\n  Shows OS, kernel version, etc."
            }
            "uptime" => {
                "⏰ uptime - Show system uptime\n  Shows how long system has been running\n  Also shows load average"
            }
            "id" => {
                "🆔 id - Show user/group IDs\n  Shows your user ID, group ID, and groups\n  id <username> : Show info for other user"
            }
            "passwd" => {
                "🔑 passwd - Change password\n  Usage: passwd\n  Changes your login password\n  Usually needs current password"
            }
            "su" | "sudo" => {
                "👑 su/sudo - Run as different user/superuser\n  sudo <command>  : Run command as root\n  su <user>       : Switch to different user\n  su -            : Switch to root"
            }
            "useradd" | "userdel" | "usermod" => {
                "👥 User management\n  useradd <name>  : Create new user\n  userdel <name>  : Delete user\n  usermod -aG <group> <user> : Add to group\n  Usually needs sudo"
            }
            "groupadd" | "groupdel" => {
                "👥 Group management\n  groupadd <name> : Create group\n  groupdel <name> : Delete group\n  Usually needs sudo"
            }
            "crontab" => {
                "⏰ crontab - Schedule tasks\n  -l  : List scheduled tasks\n  -e  : Edit schedule\n  Format: minute hour day month day-of-week command"
            }
            "at" => {
                "⏰ at - Run command at specific time\n  Usage: at 3:00 PM tomorrow\n  at> echo 'hello'\n  at> <Ctrl+D>\n  Schedules one-time tasks"
            }
            "screen" | "tmux" => {
                "💻 Terminal multiplexers\n  screen -S <name> : Create session\n  screen -r <name> : Reconnect\n  Keep processes running after disconnect"
            }
            "rsync" => {
                "🔄 rsync - Sync files/directories\n  -a  : Archive mode (preserves permissions)\n  -v  : Verbose\n  -z  : Compress during transfer\n  Usage: rsync -av source/ destination/"
            }
            "scp" => {
                "📤 scp - Secure copy over SSH\n  Usage: scp file user@host:/path/\n  scp user@host:/path/file .\n  Copy files between computers securely"
            }
            "zip" | "unzip" => {
                "📦 zip/unzip - Compress/decompress files\n  zip archive.zip file1 file2\n  unzip archive.zip\n  unzip -l archive.zip : List contents"
            }
            "gzip" | "gunzip" => {
                "📦 gzip/gunzip - Compress/decompress\n  gzip file.txt     : Creates file.txt.gz\n  gunzip file.txt.gz : Restores file.txt\n  -k : Keep original file (gzip)"
            }
            "xz" | "unxz" => {
                "📦 xz - High compression\n  xz file.txt       : Creates file.txt.xz\n  unxz file.txt.xz   : Restores file.txt\n  Better compression than gzip"
            }
            "less" | "more" => {
                "📄 less/more - View file contents\n  less <file> : View file (better than more)\n  /pattern : Search forward\n  n : Next match\n  q : Quit"
            }
            "nano" | "vim" | "emacs" => {
                "📝 Text editors\n  nano <file>  : Simple editor\n  vim <file>   : Powerful editor\n  emacs <file> : Advanced editor\n  All can create and edit text files"
            }
            "touch" => {
                "📄 touch - Create empty file or update timestamp\n  Usage: touch <filename>\n  Creates file if it doesn't exist\n  Updates modification time if it does"
            }
            "ln" => {
                "🔗 ln - Create links\n  -s  : Symbolic link (shortcut)\n  Usage: ln -s target linkname\n  ln source linkname : Hard link"
            }
            "file" => {
                "🔍 file - Determine file type\n  Usage: file <filename>\n  Shows what type of file it is\n  Useful for unknown files"
            }
            "stat" => {
                "📊 stat - Show file/directory details\n  Usage: stat <file>\n  Shows size, permissions, timestamps\n  More detailed than ls -l"
            }
            "basename" | "dirname" => {
                "📁 basename/dirname - Extract parts of path\n  basename /path/to/file.txt → file.txt\n  dirname /path/to/file.txt → /path/to\n  Useful in scripts"
            }
            "realpath" => {
                "📍 realpath - Show absolute path\n  Usage: realpath <file>\n  Converts relative paths to absolute\n  Resolves symbolic links"
            }
            "mktemp" => {
                "📄 mktemp - Create temporary file/directory\n  -d  : Create directory instead of file\n  Usage: mktemp or mktemp -d\n  Creates unique temporary names"
            }
            "split" => {
                "✂️ split - Split files into pieces\n  -b 1M : Split into 1MB chunks\n  Usage: split -b 100m largefile part_\n  Creates part_aa, part_ab, etc."
            }
            "csplit" => {
                "✂️ csplit - Split by content\n  Usage: csplit file.txt '/pattern/' '{*}' \n  Splits file at pattern matches"
            }
            "comm" => {
                "🔄 comm - Compare sorted files\n  -1  : Suppress column 1 (unique to file1)\n  -2  : Suppress column 2 (unique to file2)\n  -3  : Suppress column 3 (common lines)\n  Usage: comm file1 file2"
            }
            "join" => {
                "🔗 join - Join files on common field\n  -t ',' : Use comma as field separator\n  Usage: join file1 file2\n  Like database join operation"
            }
            "paste" => {
                "📋 paste - Merge lines from files\n  -d ',' : Use comma as delimiter\n  Usage: paste file1 file2\n  Combines corresponding lines"
            }
            "expand" | "unexpand" => {
                "↹ expand/unexpand - Convert tabs/spaces\n  expand -t 4 file : Convert tabs to 4 spaces\n  unexpand -t 4 file : Convert spaces to tabs"
            }
            "tr" => {
                "🔄 tr - Translate characters\n  'a-z' 'A-Z' : Convert to uppercase\n  -d 'abc' : Delete characters a,b,c\n  Usage: command | tr 'a-z' 'A-Z'"
            }
            "cut" => {
                "✂️ cut - Extract columns from text\n  -d ',' -f 1 : Get first comma-separated field\n  -c 1-10 : Get characters 1 through 10\n  Usage: command | cut -d ' ' -f 1"
            }
            "awk" => {
                "🔧 awk - Text processing\n  '{print $1}' : Print first column\n  '/pattern/ {print}' : Print lines matching pattern\n  Powerful text manipulation tool"
            }
            "sed" => {
                "🔧 sed - Stream editor\n  's/old/new/g' : Replace text\n  '/pattern/d' : Delete lines\n  Usage: sed 's/hello/hi/g' file.txt"
            }
            "xargs" => {
                "🔧 xargs - Build command from input\n  -n 1 : One argument per command\n  Usage: echo 'file1 file2' | xargs rm\n  Converts input into command arguments"
            }
            "tee" => {
                "📋 tee - Copy output to files and screen\n  Usage: command | tee output.txt\n  Shows output on screen AND saves to file"
            }
            "yes" => {
                "🔁 yes - Output string repeatedly\n  Usage: yes 'y' | command\n  Automatically answers 'y' to prompts\n  yes | head -10 : Print 'y' 10 times"
            }
            "seq" => {
                "🔢 seq - Generate sequences\n  Usage: seq 1 10\n  seq 1 2 20 : Count by 2s\n  Generates number sequences"
            }
            "factor" => {
                "🔢 factor - Factorize numbers\n  Usage: factor 12345\n  Shows prime factors of numbers"
            }
            "bc" => {
                "🔢 bc - Calculator\n  Usage: echo '2+2' | bc\n  bc : Interactive calculator\n  Supports advanced math"
            }
            "time" => {
                "⏱️ time - Measure command execution time\n  Usage: time command\n  Shows real, user, and system time"
            }
            "timeout" => {
                "⏱️ timeout - Run command with time limit\n  Usage: timeout 10s command\n  Kills command after 10 seconds"
            }
            "watch" => {
                "👀 watch - Run command repeatedly\n  -n 2 : Run every 2 seconds\n  Usage: watch -n 1 'ls -la'\n  Monitor changes over time"
            }
            "sleep" => {
                "😴 sleep - Pause for specified time\n  Usage: sleep 5s, sleep 1m, sleep 1h\n  Pauses script execution"
            }
            "wait" => {
                "⏳ wait - Wait for background processes\n  Usage: wait\n  wait <PID> : Wait for specific process\n  Used in shell scripts"
            }
            "jobs" => {
                "💼 jobs - Show background jobs\n  Shows running/stopped background processes\n  %1 : Refer to job number 1"
            }
            "fg" | "bg" => {
                "💼 fg/bg - Foreground/background jobs\n  fg %1 : Bring job 1 to foreground\n  bg %1 : Send job 1 to background\n  Control background processes"
            }
            "disown" => {
                "💼 disown - Remove job from shell control\n  Usage: disown %1\n  Job continues after shell exits"
            }
            "nice" | "renice" => {
                "⚡ nice/renice - Set process priority\n  nice -n 10 command : Lower priority\n  renice -n -5 <PID> : Higher priority\n  -20 to 19 range (lower = higher priority)"
            }
            "ionice" => {
                "💿 ionice - Set I/O priority\n  -c 3 : Idle I/O class\n  -c 2 -n 7 : Best-effort class\n  Controls disk I/O priority"
            }
            "taskset" => {
                "🖥️ taskset - Set CPU affinity\n  -c 0-3 : Use CPUs 0,1,2,3\n  Usage: taskset -c 0 command\n  Bind process to specific CPUs"
            }
            "chrt" => {
                "⚡ chrt - Set scheduling policy\n  --rr : Round-robin scheduling\n  --fifo : First-in-first-out\n  Advanced process scheduling"
            }
            "strace" => {
                "🔍 strace - Trace system calls\n  -p <PID> : Trace running process\n  -e trace=open : Trace only open calls\n  Shows what system calls a program makes"
            }
            "ltrace" => {
                "🔍 ltrace - Trace library calls\n  -p <PID> : Trace running process\n  Shows library function calls"
            }
            "gdb" => {
                "🐛 gdb - GNU debugger\n  gdb program : Debug program\n  run : Start execution\n  break main : Set breakpoint\n  Powerful debugging tool"
            }
            "valgrind" => {
                "🐛 valgrind - Memory debugger\n  --leak-check=full : Check memory leaks\n  Usage: valgrind program\n  Finds memory errors and leaks"
            }
            "perf" => {
                "📊 perf - Performance profiler\n  stat : Basic statistics\n  record : Record performance data\n  report : Show performance report\n  Linux performance analysis tool"
            }
            "dmesg" => {
                "📋 dmesg - Kernel message buffer\n  -T : Human readable timestamps\n  Shows kernel log messages\n  Useful for hardware/driver issues"
            }
            "syslog" | "journalctl" => {
                "📋 System logging\n  journalctl -u service : Service logs\n  journalctl -f : Follow new messages\n  journalctl --since '1 hour ago'\n  View system and service logs"
            }
            "logger" => {
                "📝 logger - Add messages to system log\n  Usage: logger 'message'\n  logger -p local0.info 'message'\n  Write to system log from scripts"
            }
            "lsof" => {
                "🔍 lsof - List open files\n  -p <PID> : Files open by process\n  -i : Network connections\n  -u <user> : Files open by user\n  Shows all open files and network connections"
            }
            "fuser" => {
                "🔍 fuser - Find processes using file\n  -k : Kill processes\n  Usage: fuser -k /path/to/file\n  Shows/kills processes using a file"
            }
            "vmstat" => {
                "📊 vmstat - Virtual memory statistics\n  1 : Update every second\n  Shows memory, CPU, I/O statistics"
            }
            "iostat" => {
                "💿 iostat - I/O statistics\n  -x : Extended statistics\n  1 : Update every second\n  Shows disk I/O performance"
            }
            "sar" => {
                "📊 sar - System activity report\n  -u : CPU usage\n  -r : Memory usage\n  -d : Disk I/O\n  Collects and reports system activity"
            }
            "mpstat" => {
                "📊 mpstat - Multi-processor statistics\n  -P ALL : All CPUs\n  1 : Update every second\n  Shows per-CPU statistics"
            }
            "pstree" => {
                "🌳 pstree - Process tree\n  -p : Show PIDs\n  Shows process hierarchy\n  Visual representation of process relationships"
            }
            "pgrep" => {
                "🔍 pgrep - Find processes by name\n  Usage: pgrep firefox\n  Shows PIDs of matching processes"
            }
            "pkill" => {
                "💀 pkill - Kill processes by name\n  Usage: pkill firefox\n  Kills all processes matching name"
            }
            "pidof" => {
                "🔍 pidof - Find PID of program\n  Usage: pidof firefox\n  Shows process ID of running program"
            }
            "nohup" => {
                "💼 nohup - Run command immune to hangups\n  Usage: nohup command &\n  Process continues after logout"
            }
            "setsid" => {
                "💼 setsid - Run program in new session\n  Usage: setsid command\n  Creates new process group and session"
            }
            "daemonize" => {
                "👻 daemonize - Run as daemon\n  Usage: daemonize command\n  Detach from terminal, run in background"
            }
            "trap" => {
                "🪤 trap - Catch signals in scripts\n  trap 'echo cleanup' EXIT\n  trap 'handler' INT TERM\n  Handle signals and cleanup"
            }
            "ulimit" => {
                "⚙️ ulimit - Set resource limits\n  -u 100 : Max user processes\n  -v 1000000 : Max virtual memory\n  Control resource usage limits"
            }
            "getconf" => {
                "⚙️ getconf - Get configuration values\n  Usage: getconf PAGE_SIZE\n  Shows system configuration values"
            }
            "locale" => {
                "🌍 locale - Show locale settings\n  Shows language and regional settings\n  locale -a : List all available locales"
            }
            "tzselect" | "timedatectl" => {
                "🕐 Time zone management\n  timedatectl set-timezone America/New_York\n  tzselect : Interactive timezone selection\n  Set system timezone"
            }
            "hostname" => {
                "💻 hostname - Show/set system hostname\n  hostname : Show current hostname\n  hostname newname : Set new hostname"
            }
            "dnsdomainname" => {
                "🌐 dnsdomainname - Show DNS domain name\n  Shows system's DNS domain\n  Part of hostname after first dot"
            }
            "domainname" => {
                "🌐 domainname - Show/set NIS domain\n  Shows NIS/YP domain name\n  Used in network information services"
            }
            "nisdomainname" => {
                "🌐 nisdomainname - Show/set NIS domain\n  Same as domainname\n  Network Information Service domain"
            }
            "ypdomainname" => {
                "🌐 ypdomainname - Yellow Pages domain\n  Same as domainname\n  Legacy name for NIS"
            }
            "arch" => {
                "💻 arch - Show machine architecture\n  Shows CPU architecture (x86_64, arm64, etc.)\n  Same as uname -m"
            }
            "nproc" => {
                "🖥️ nproc - Show number of CPUs\n  Shows available CPU cores\n  --all : Include offline CPUs"
            }
            "lscpu" => {
                "🖥️ lscpu - CPU information\n  Shows detailed CPU architecture information\n  Cores, sockets, threads, cache, etc."
            }
            "lsmem" => {
                "🧠 lsmem - Memory information\n  Shows memory block information\n  --summary : Brief summary"
            }
            "lsblk" => {
                "💿 lsblk - List block devices\n  Shows disk and partition information\n  -f : Show filesystem types"
            }
            "blkid" => {
                "💿 blkid - Show block device attributes\n  Shows UUID, filesystem type, etc.\n  Useful for /etc/fstab configuration"
            }
            "findmnt" => {
                "💿 findmnt - Find mounted filesystems\n  Shows all mounted filesystems\n  -t ext4 : Filter by type"
            }
            "mountpoint" => {
                "💿 mountpoint - Check if directory is mount point\n  Usage: mountpoint /mnt\n  Returns success if directory is a mount point"
            }
            "losetup" => {
                "💿 losetup - Set up loop devices\n  -f : Find free loop device\n  losetup /dev/loop0 file.iso\n  Mount ISO files or disk images"
            }
            "swapon" | "swapoff" => {
                "💾 Swap management\n  swapon /dev/sda2 : Enable swap\n  swapoff /dev/sda2 : Disable swap\n  swapon -s : Show swap status"
            }
            "mkswap" => {
                "💾 mkswap - Set up swap area\n  Usage: mkswap /dev/sda2\n  Format partition for use as swap"
            }
            "fdisk" => {
                "💿 fdisk - Disk partition table manipulator\n  -l : List partitions\n  Interactive partitioning tool\n  Create, delete, modify partitions"
            }
            "parted" => {
                "💿 parted - Advanced partitioning tool\n  print : Show partition table\n  mkpart : Create partition\n  rm : Remove partition\n  More advanced than fdisk"
            }
            "mkfs" => {
                "💿 mkfs - Make filesystem\n  mkfs.ext4 /dev/sda1 : Create ext4 filesystem\n  mkfs.vfat /dev/sda1 : Create FAT filesystem\n  Format partitions"
            }
            "fsck" => {
                "💿 fsck - Filesystem check and repair\n  fsck /dev/sda1 : Check filesystem\n  -y : Answer yes to all questions\n  Repair filesystem errors"
            }
            "tune2fs" => {
                "💿 tune2fs - Adjust ext2/ext3/ext4 filesystem\n  -l : Show filesystem information\n  -c 30 : Check every 30 mounts\n  Adjust filesystem parameters"
            }
            "dumpe2fs" => {
                "💿 dumpe2fs - Dump ext2/ext3/ext4 filesystem info\n  Usage: dumpe2fs /dev/sda1\n  Shows detailed filesystem information"
            }
            "resize2fs" => {
                "💿 resize2fs - Resize ext2/ext3/ext4 filesystem\n  Usage: resize2fs /dev/sda1\n  Grow or shrink filesystem size"
            }
            "e2fsck" => {
                "💿 e2fsck - Check ext2/ext3/ext4 filesystem\n  Same as fsck for ext filesystems\n  More detailed checking and repair"
            }
            "debugfs" => {
                "💿 debugfs - Ext filesystem debugger\n  debugfs /dev/sda1\n  Interactive filesystem debugging tool\n  Advanced filesystem manipulation"
            }
            "xfs_info" => {
                "💿 xfs_info - Show XFS filesystem info\n  Usage: xfs_info /dev/sda1\n  Shows XFS filesystem parameters"
            }
            "xfs_repair" => {
                "💿 xfs_repair - Repair XFS filesystem\n  Usage: xfs_repair /dev/sda1\n  Repair corrupted XFS filesystem"
            }
            "btrfs" => {
                "💿 btrfs - Btrfs filesystem utilities\n  filesystem show : Show btrfs filesystems\n  subvolume list / : List subvolumes\n  Advanced filesystem with snapshots"
            }
            "zfs" => {
                "💿 zfs - ZFS filesystem management\n  list : Show ZFS datasets\n  create tank/data : Create dataset\n  snapshot tank/data@backup\n  Enterprise-grade filesystem"
            }
            "mdadm" => {
                "💿 mdadm - Software RAID management\n  --detail /dev/md0 : Show RAID array info\n  --create /dev/md0 : Create RAID array\n  Manage software RAID arrays"
            }
            "cryptsetup" => {
                "🔐 cryptsetup - Disk encryption\n  luksFormat /dev/sda1 : Encrypt partition\n  luksOpen /dev/sda1 secret : Open encrypted device\n  Linux Unified Key Setup"
            }
            "luks" => {
                "🔐 LUKS - Linux Unified Key Setup\n  Part of cryptsetup\n  Standard for disk encryption on Linux"
            }
            "gpg" => {
                "🔐 gpg - GNU Privacy Guard\n  --gen-key : Generate key pair\n  --encrypt file : Encrypt file\n  --decrypt file.gpg : Decrypt file\n  GNU implementation of OpenPGP"
            }
            "openssl" => {
                "🔐 openssl - SSL/TLS toolkit\n  rand -base64 32 : Generate random data\n  req -new -x509 : Create self-signed certificate\n  Comprehensive cryptography toolkit"
            }
            "ssh-keygen" => {
                "🔐 ssh-keygen - Generate SSH keys\n  -t rsa : Generate RSA key\n  -t ed25519 : Generate Ed25519 key\n  Create SSH key pairs for authentication"
            }
            "ssh-copy-id" => {
                "🔐 ssh-copy-id - Copy SSH keys to server\n  Usage: ssh-copy-id user@host\n  Installs your public key on remote server\n  Enables passwordless SSH login"
            }
            "ssh-agent" => {
                "🔐 ssh-agent - SSH key manager\n  ssh-agent bash : Start agent\n  ssh-add : Add keys to agent\n  Manages SSH keys in memory"
            }
            "ssh-add" => {
                "🔐 ssh-add - Add SSH keys to agent\n  ssh-add ~/.ssh/id_rsa : Add specific key\n  ssh-add -l : List loaded keys\n  Add private keys to ssh-agent"
            }
            "sshd" => {
                "🔐 sshd - SSH daemon\n  /usr/sbin/sshd : SSH server daemon\n  Listens for SSH connections\n  Usually started by systemd"
            }
            "iptables" => {
                "🔥 iptables - Firewall rules\n  -L : List rules\n  -A INPUT -p tcp --dport 22 -j ACCEPT\n  Configure netfilter firewall rules"
            }
            "ufw" => {
                "🔥 ufw - Uncomplicated Firewall\n  status : Show status\n  allow 22 : Allow SSH\n  enable : Enable firewall\n  Simpler interface to iptables"
            }
            "firewalld" => {
                "🔥 firewalld - Dynamic firewall\n  --state : Show status\n  --add-service=ssh : Allow SSH\n  --reload : Reload rules\n  Modern firewall management"
            }
            "nftables" => {
                "🔥 nftables - Netfilter tables\n  list ruleset : Show all rules\n  Successor to iptables\n  More efficient and flexible"
            }
            "tcpdump" => {
                "📡 tcpdump - Network packet analyzer\n  -i eth0 : Listen on interface\n  port 80 : Filter by port\n  -w capture.pcap : Save to file\n  Capture and analyze network traffic"
            }
            "wireshark" => {
                "📡 wireshark - Network protocol analyzer\n  GUI version of tcpdump\n  Analyze network traffic with GUI\n  Powerful protocol dissection"
            }
            "nmap" => {
                "📡 nmap - Network mapper\n  -sP 192.168.1.0/24 : Ping scan network\n  -p 80,443 : Scan specific ports\n  -A : Aggressive scan with OS detection\n  Network discovery and security auditing"
            }
            "netstat" => {
                "📡 netstat - Network statistics\n  -tlnp : Show listening TCP ports\n  -rn : Show routing table\n  -i : Show network interfaces\n  Network connection information"
            }
            "ss" => {
                "📡 ss - Socket statistics\n  -tlnp : Show listening TCP sockets\n  -rn : Show routing table\n  Modern replacement for netstat"
            }
            "route" => {
                "📡 route - Show/manipulate routing table\n  -n : Numeric output\n  add default gw 192.168.1.1 : Add default route\n  Legacy routing table management"
            }
            "traceroute" => {
                "📡 traceroute - Trace packet route\n  Usage: traceroute host\n  Shows path packets take to destination\n  Useful for network troubleshooting"
            }
            "mtr" => {
                "📡 mtr - Network diagnostic tool\n  Usage: mtr host\n  Combines traceroute and ping\n  Real-time network diagnostics"
            }
            "dig" => {
                "🌐 dig - DNS lookup\n  Usage: dig google.com\n  @8.8.8.8 : Use specific DNS server\n  Shows DNS records and resolution"
            }
            "nslookup" => {
                "🌐 nslookup - DNS query tool\n  Usage: nslookup google.com\n  Interactive DNS queries\n  Legacy DNS lookup tool"
            }
            "host" => {
                "🌐 host - DNS lookup utility\n  Usage: host google.com\n  Shows IP addresses for hostnames\n  Simple DNS lookups"
            }
            "whois" => {
                "🌐 whois - Domain registration info\n  Usage: whois google.com\n  Shows domain registration details\n  Owner, registrar, dates, etc."
            }
            "lynx" | "links" | "elinks" => {
                "🌐 Text-based web browsers\n  lynx google.com : Browse web in terminal\n  Useful for headless servers\n  No graphics, pure text"
            }
            "ftp" => {
                "📁 ftp - File Transfer Protocol\n  ftp ftp.example.com\n  get file.txt : Download file\n  put file.txt : Upload file\n  Legacy file transfer protocol"
            }
            "sftp" => {
                "📁 sftp - Secure File Transfer\n  sftp user@host\n  get file.txt : Download file\n  put file.txt : Upload file\n  Secure version of FTP over SSH"
            }
            "nc" | "netcat" => {
                "📡 netcat - Networking utility\n  nc -l 1234 : Listen on port 1234\n  nc host 1234 : Connect to port 1234\n  Swiss army knife of networking"
            }
            "socat" => {
                "📡 socat - Multipurpose relay\n  socat TCP-LISTEN:1234 TCP:host:80\n  Advanced netcat replacement\n  Create network connections and tunnels"
            }
            "telnet" => {
                "📡 telnet - Connect to remote host\n  telnet host 23 : Connect to telnet server\n  telnet host 80 : Manual HTTP requests\n  Legacy remote login protocol"
            }
            "rsh" | "rlogin" => {
                "📡 Remote shell commands\n  rsh host command : Run command remotely\n  rlogin host : Login remotely\n  Legacy remote execution tools"
            }
            "byobu" => {
                "💻 byobu - Enhanced terminal multiplexer\n  Wrapper around tmux/screen\n  Pre-configured with useful features\n  Easy to use terminal management"
            }
            "script" => {
                "📝 script - Record terminal session\n  script logfile.txt : Start recording\n  exit : Stop recording\n  Records everything typed and output"
            }
            "scriptreplay" => {
                "📝 scriptreplay - Replay recorded session\n  scriptreplay timingfile logfile\n  Replay terminal session with timing\n  Play back recorded sessions"
            }
            "tput" => {
                "🎨 tput - Terminal capabilities\n  tput clear : Clear screen\n  tput cup 10 20 : Move cursor\n  tput setaf 1 : Set foreground color\n  Control terminal appearance"
            }
            "stty" => {
                "⚙️ stty - Terminal settings\n  stty -a : Show all settings\n  stty sane : Reset to sane defaults\n  Configure terminal behavior"
            }
            "reset" => {
                "🔄 reset - Reset terminal\n  reset : Reset terminal settings\n  clear : Just clear screen\n  Fix corrupted terminal display"
            }
            "clear" => {
                "🧹 clear - Clear terminal screen\n  clear : Clear screen and scrollback\n  Ctrl+L : Clear screen (in most shells)\n  Clean terminal display"
            }
            "resize" => {
                "📐 resize - Set terminal size\n  resize : Update LINES and COLUMNS\n  Useful after terminal resize\n  Update shell's idea of terminal size"
            }
            "tty" => {
                "💻 tty - Show terminal device\n  tty : Show current terminal device\n  Shows /dev/pts/X or /dev/ttyX\n  Which terminal you're using"
            }
            "mesg" => {
                "💬 mesg - Control write access to terminal\n  mesg y : Allow write access\n  mesg n : Deny write access\n  Control who can write to your terminal"
            }
            "wall" => {
                "📢 wall - Write to all users\n  wall 'message' : Send message to all\n  Usually requires root\n  Broadcast messages to all logged-in users"
            }
            "write" => {
                "💬 write - Write to specific user\n  write user tty : Send message to user\n  Ctrl+D : End message\n  Send messages to specific users"
            }
            "talk" => {
                "💬 talk - Interactive chat\n  talk user@host : Start chat\n  Legacy interactive chat program\n  Real-time text chat between users"
            }
            "finger" => {
                "👤 finger - User information\n  finger user : Show user info\n  finger @host : Show logged-in users\n  Show user information and status"
            }
            "w" => {
                "👥 w - Show who is logged in\n  w : Show logged-in users and activity\n  Shows user, terminal, login time, activity\n  More detailed than who"
            }
            "who" => {
                "👥 who - Show logged-in users\n  who : Show logged-in users\n  who am i : Show your own info\n  Basic logged-in user information"
            }
            "last" => {
                "📜 last - Show login history\n  last : Show recent logins/logouts\n  last -10 : Show last 10 entries\n  Login history from /var/log/wtmp"
            }
            "lastlog" => {
                "📜 lastlog - Show last login times\n  lastlog : Show last login for all users\n  Shows when each user last logged in\n  From /var/log/lastlog"
            }
            "ac" => {
                "⏰ ac - Show connect time\n  ac : Show total connect time\n  ac -p : Per-user connect time\n  Show user connection statistics"
            }
            "tload" => {
                "📊 tload - Show system load\n  tload : Graphical load average\n  Shows system load over time\n  Text-based load graph"
            }
            "isag" => {
                "📊 isag - Interactive system activity graph\n  isag : Interactive performance graphs\n  Visual system performance monitoring\n  Part of sysstat package"
            }
            _ => {
                "❓ Command not found in database\n  Try: man <command> (if available)\n  Or: <command> --help\n  Or: whatis <command>"
            }
        };

        self.add_line(&explanation, false, false);
    }

    fn update_autocomplete(&mut self) {
        self.refresh_command_cache();

        // Get the current word being typed (last word in input)
        let words: Vec<&str> = self.input_buffer.split_whitespace().collect();
        let current_word = if self.input_buffer.ends_with(' ') {
            ""
        } else {
            words.last().map_or("", |&word| word)
        };

        // Find matching suggestions
        let mut suggestions = Vec::new();

        // If it's the first word, match against commands
        if words.len() <= 1 {
            if current_word.is_empty() {
                // Show recent commands when input is empty
                for cmd in self.command_history.iter().rev().take(10) {
                    if let Some(first_word) = cmd.split_whitespace().next() {
                        suggestions.push(first_word.to_string());
                    }
                }
            } else {
                // Get suggestions from different sources
                let mut all_candidates = Vec::new();

                // Common commands
                for cmd in &self.common_commands {
                    if cmd.starts_with(current_word) {
                        all_candidates.push((cmd.clone(), 90)); // High priority
                    }
                }

                // PATH commands
                for cmd in &self.path_commands {
                    if cmd.starts_with(current_word) {
                        all_candidates.push((cmd.clone(), 80)); // Medium-high priority
                    }
                }

                // Package commands
                if let Some(package_cmds) = self.command_cache.get("packages") {
                    for cmd in package_cmds {
                        if cmd.starts_with(current_word) {
                            all_candidates.push((cmd.clone(), 70)); // Medium priority
                        }
                    }
                }

                // Command history
                let history_suggestions = self.get_command_history_suggestions(current_word);
                for cmd in history_suggestions {
                    all_candidates.push((cmd, 85)); // High-medium priority
                }

                // Fuzzy matching if enabled
                if self.fuzzy_enabled {
                    let mut fuzzy_candidates = Vec::new();

                    // Check all sources for fuzzy matches
                    for cmd in &self.common_commands {
                        let score = self.fuzzy_match(current_word, cmd);
                        if score > 0 {
                            fuzzy_candidates.push((cmd.clone(), score));
                        }
                    }

                    for cmd in &self.path_commands {
                        let score = self.fuzzy_match(current_word, cmd);
                        if score > 0 {
                            fuzzy_candidates.push((cmd.clone(), score));
                        }
                    }

                    if let Some(package_cmds) = self.command_cache.get("packages") {
                        for cmd in package_cmds {
                            let score = self.fuzzy_match(current_word, cmd);
                            if score > 0 {
                                fuzzy_candidates.push((cmd.clone(), score));
                            }
                        }
                    }

                    // Sort fuzzy candidates by score and take top ones
                    fuzzy_candidates.sort_by(|a, b| b.1.cmp(&a.1));
                    for (cmd, _) in fuzzy_candidates.into_iter().take(5) {
                        if !all_candidates.iter().any(|(c, _)| c == &cmd) {
                            all_candidates.push((cmd, 60)); // Lower priority for fuzzy matches
                        }
                    }
                }

                // Sort by priority and deduplicate
                all_candidates.sort_by(|a, b| b.1.cmp(&a.1));
                let mut seen = std::collections::HashSet::new();

                for (cmd, _) in all_candidates {
                    if !seen.contains(&cmd) {
                        suggestions.push(cmd.clone());
                        seen.insert(cmd);
                        if suggestions.len() >= 20 { // Limit suggestions
                            break;
                        }
                    }
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
                        if flag.starts_with(current_word) {
                            suggestions.push(flag.clone());
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
                            if file_name.starts_with(current_word) {
                                // Add directory indicator if it's a directory
                                let suggestion = if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                                    format!("{}/", file_name)
                                } else {
                                    file_name.to_string()
                                };
                                suggestions.push(suggestion);
                            }
                        }
                    }
                }
            }
        }

        // Update suggestions
        self.autocomplete_suggestions = suggestions;
        self.show_autocomplete = !self.autocomplete_suggestions.is_empty();
        self.autocomplete_index = -1;
    }

    fn scan_path_commands(&mut self) {
        let mut path_commands = Vec::new();

        if let Ok(path_var) = env::var("PATH") {
            for dir in env::split_paths(&path_var) {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_file() || file_type.is_symlink() {
                                if let Some(name) = entry.file_name().to_str() {
                                    // Skip if it contains spaces or is already in common_commands
                                    if !name.contains(' ') && !self.common_commands.contains(&name.to_string()) {
                                        // Check if it's actually executable
                                        if self.is_executable(&dir.join(name)) {
                                            path_commands.push(name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort and deduplicate
        path_commands.sort();
        path_commands.dedup();
        self.path_commands = path_commands;
        self.last_path_scan = Instant::now();
    }

    fn is_executable(&self, path: &std::path::Path) -> bool {
        if let Ok(metadata) = path.metadata() {
            // Check if it's executable by owner, group, or others
            metadata.permissions().mode() & 0o111 != 0
        } else {
            false
        }
    }

    fn fuzzy_match(&self, query: &str, candidate: &str) -> i32 {
        if query.is_empty() {
            return 0;
        }

        let query_lower = query.to_lowercase();
        let candidate_lower = candidate.to_lowercase();

        // Exact prefix match gets highest score
        if candidate_lower.starts_with(&query_lower) {
            return 100 - candidate.len() as i32;
        }

        // Contains match gets medium score
        if candidate_lower.contains(&query_lower) {
            return 50 - candidate.len() as i32;
        }

        // Fuzzy matching: check if all characters of query appear in order
        let mut query_chars = query_lower.chars();
        let mut current_char = query_chars.next();

        for c in candidate_lower.chars() {
            if let Some(qc) = current_char {
                if c == qc {
                    current_char = query_chars.next();
                }
            }
        }

        if current_char.is_none() {
            // All characters found in order, but not consecutive
            return 25 - candidate.len() as i32;
        }

        0 // No match
    }

    fn get_command_history_suggestions(&self, prefix: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Get recent commands from history
        for cmd in self.command_history.iter().rev() {
            if let Some(first_word) = cmd.split_whitespace().next() {
                if first_word.starts_with(prefix) && !seen.contains(first_word) {
                    suggestions.push(first_word.to_string());
                    seen.insert(first_word.to_string());
                    if suggestions.len() >= 5 {
                        break;
                    }
                }
            }
        }

        suggestions
    }

    fn get_package_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();

        // Try different package managers
        let package_managers = vec![
            ("dpkg", vec!["-l"]),
            ("rpm", vec!["-qa"]),
            ("pacman", vec!["-Q"]),
            ("brew", vec!["list"]),
        ];

        for (pm, args) in package_managers {
            if let Ok(output) = Command::new(pm).args(&args).output() {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    // Parse package names and extract command names
                    for line in stdout.lines() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if !parts.is_empty() {
                            let package = parts[0];
                            // Extract command name from package name (simple heuristic)
                            if let Some(cmd_name) = self.extract_command_from_package(package) {
                                if !self.common_commands.contains(&cmd_name) {
                                    commands.push(cmd_name);
                                }
                            }
                        }
                    }
                }
            }
        }

        commands.sort();
        commands.dedup();
        commands
    }

    fn extract_command_from_package(&self, package: &str) -> Option<String> {
        // Simple heuristics to extract command names from package names
        let package_lower = package.to_lowercase();

        // Remove version numbers and architecture suffixes
        let clean_package = package_lower
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or(package);

        // Common patterns
        if clean_package.starts_with("lib") {
            return None; // Skip libraries
        }

        Some(clean_package.to_string())
    }

    fn refresh_command_cache(&mut self) {
        // Refresh PATH commands if it's been more than 30 seconds
        if self.last_path_scan.elapsed() > Duration::from_secs(30) {
            self.scan_path_commands();
        }

        // Cache package commands
        if !self.command_cache.contains_key("packages") {
            let package_cmds = self.get_package_commands();
            self.command_cache.insert("packages".to_string(), package_cmds);
        }
    }

    fn apply_autocomplete(&mut self) -> bool {
        if self.autocomplete_suggestions.is_empty() {
            return false;
        }

        // If only one suggestion, apply it directly
        if self.autocomplete_suggestions.len() == 1 {
            self.autocomplete_index = 0;
        } else {
            // Cycle through suggestions
            if self.autocomplete_index < 0 {
                self.autocomplete_index = 0;
            } else {
                self.autocomplete_index = (self.autocomplete_index + 1) % self.autocomplete_suggestions.len() as isize;
            }
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

            // If it's a flag or command, add a space for easier continuation
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
                if self.selection_start.is_some() && self.selection_end.is_some() {
                    // Delete selection if exists
                    self.delete_selection();
                    self.update_autocomplete();
                } else if self.cursor_pos > 0 {
                    self.input_buffer.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                    self.update_autocomplete();
                }
            }
            egui::Key::Delete => {
                if self.selection_start.is_some() && self.selection_end.is_some() {
                    // Delete selection if exists
                    self.delete_selection();
                    self.update_autocomplete();
                } else if self.cursor_pos < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_pos);
                    self.update_autocomplete();
                }
            }
            egui::Key::ArrowLeft => {
                if modifiers.shift {
                    // Shift+Left: Extend selection
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor_pos);
                    }
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                        self.selection_end = Some(self.cursor_pos);
                    }
                } else {
                    // Left: Move cursor and clear selection
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                    }
                    self.selection_start = None;
                    self.selection_end = None;
                }
            }
            egui::Key::ArrowRight => {
                if modifiers.shift {
                    // Shift+Right: Extend selection
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor_pos);
                    }
                    if self.cursor_pos < self.input_buffer.len() {
                        self.cursor_pos += 1;
                        self.selection_end = Some(self.cursor_pos);
                    }
                } else {
                    // Right: Move cursor and clear selection
                    if self.cursor_pos < self.input_buffer.len() {
                        self.cursor_pos += 1;
                    }
                    self.selection_start = None;
                    self.selection_end = None;
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
                if modifiers.shift {
                    // Shift+Home: Select from cursor to beginning
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor_pos);
                    }
                    self.selection_end = Some(0);
                    self.cursor_pos = 0;
                } else {
                    // Home: Go to beginning and clear selection
                    self.cursor_pos = 0;
                    self.selection_start = None;
                    self.selection_end = None;
                }
            }
            egui::Key::End => {
                if modifiers.shift {
                    // Shift+End: Select from cursor to end
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor_pos);
                    }
                    self.selection_end = Some(self.input_buffer.len());
                    self.cursor_pos = self.input_buffer.len();
                } else {
                    // End: Go to end and clear selection
                    self.cursor_pos = self.input_buffer.len();
                    self.selection_start = None;
                    self.selection_end = None;
                }
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
            egui::Key::Space if modifiers.ctrl => {
                // Ctrl+Space: Toggle autocomplete suggestions
                if self.show_autocomplete {
                    self.show_autocomplete = false;
                } else {
                    self.update_autocomplete();
                }
            }
            egui::Key::F if modifiers.ctrl => {
                // Ctrl+F: Toggle fuzzy matching
                self.fuzzy_enabled = !self.fuzzy_enabled;
                if self.show_autocomplete {
                    self.update_autocomplete();
                }
                self.add_line(&format!("Fuzzy matching {}", if self.fuzzy_enabled { "enabled" } else { "disabled" }), false, false);
            }
            egui::Key::C if modifiers.ctrl && modifiers.shift => {
                // Ctrl+Shift+C: Copy selected text or current line (legacy shortcut)
                if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                    let selected_text = if start <= end {
                        self.input_buffer[start..end].to_string()
                    } else {
                        self.input_buffer[end..start].to_string()
                    };
                    if !selected_text.is_empty() {
                        self.pending_copy = Some(selected_text);
                    }
                } else {
                    // Copy entire input buffer if no selection
                    if !self.input_buffer.is_empty() {
                        self.pending_copy = Some(self.input_buffer.clone());
                    }
                }
            }
            egui::Key::C if modifiers.ctrl => {
                // Ctrl+C - copy selected text or interrupt
                if self.selection_start.is_some() && self.selection_end.is_some() {
                    // Copy selected text
                    if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                        let selected_text = if start <= end {
                            self.input_buffer[start..end].to_string()
                        } else {
                            self.input_buffer[end..start].to_string()
                        };
                        if !selected_text.is_empty() {
                            self.pending_copy = Some(selected_text);
                        }
                    }
                } else if !self.input_buffer.is_empty() {
                    // Copy entire line if no selection
                    self.pending_copy = Some(self.input_buffer.clone());
                } else {
                    // No selection and empty buffer - interrupt command
                    self.add_line("^C", false, false);
                    self.input_buffer.clear();
                    self.cursor_pos = 0;
                    self.show_prompt();
                }
            }
            egui::Key::X if modifiers.ctrl => {
                // Ctrl+X - cut selected text
                if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                    let selected_text = if start <= end {
                        self.input_buffer[start..end].to_string()
                    } else {
                        self.input_buffer[end..start].to_string()
                    };
                    if !selected_text.is_empty() {
                        self.pending_copy = Some(selected_text);
                        self.delete_selection();
                        self.update_autocomplete();
                    }
                }
            }
            egui::Key::A if modifiers.ctrl => {
                // Ctrl+A: Select all
                self.selection_start = Some(0);
                self.selection_end = Some(self.input_buffer.len());
            }
            _ => {
                if modifiers.ctrl {
                    match key {
                        egui::Key::V => {
                            // Ctrl+V - paste from clipboard
                            // We'll handle this in the update method to access ctx
                            self.pending_paste = true;
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

    fn delete_selection(&mut self) {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (min_pos, max_pos) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            
            self.input_buffer.replace_range(min_pos..max_pos, "");
            self.cursor_pos = min_pos;
            self.selection_start = None;
            self.selection_end = None;
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
                        // Clear selection when typing
                        if self.selection_start.is_some() && self.selection_end.is_some() {
                            self.delete_selection();
                        }
                        for ch in text.chars() {
                            if ch.is_control() || ch == '\n' || ch == '\r' {
                                continue;
                            }
                            self.input_buffer.insert(self.cursor_pos, ch);
                            self.cursor_pos += 1;
                        }
                        // Update autocomplete immediately when typing
                        self.update_autocomplete();
                        self.selection_start = None;
                        self.selection_end = None;
                    }
                    egui::Event::PointerButton { pos: _, button: egui::PointerButton::Primary, pressed: true, .. } => {
                        // Handle mouse click for cursor positioning
                        // For now, we'll just clear selection on click
                        self.selection_start = None;
                        self.selection_end = None;
                    }
                    _ => {}
                }
            }
        });

        // Handle pending clipboard operations
        if let Some(text) = self.pending_copy.take() {
            ctx.copy_text(text.clone());
            self.clipboard_content = text; // Store internally too
        }
        if self.pending_paste {
            self.pending_paste = false;
            // Use our internal clipboard content
            if !self.clipboard_content.is_empty() {
                // Clear selection if any
                if self.selection_start.is_some() && self.selection_end.is_some() {
                    self.delete_selection();
                }
                // Insert clipboard text at cursor position
                for ch in self.clipboard_content.chars() {
                    if ch != '\n' && ch != '\r' { // Avoid multiline paste
                        self.input_buffer.insert(self.cursor_pos, ch);
                        self.cursor_pos += 1;
                    }
                }
                self.update_autocomplete();
                self.selection_start = None;
                self.selection_end = None;
            }
        }

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

                                                // Show the input with cursor and selection
                                                ui.horizontal(|ui| {
                                                    if let (Some(sel_start), Some(sel_end)) = (self.selection_start, self.selection_end) {
                                                        let (start, end) = if sel_start <= sel_end {
                                                            (sel_start, sel_end)
                                                        } else {
                                                            (sel_end, sel_start)
                                                        };
                                                        
                                                        // Render unselected part before selection
                                                        if start > 0 {
                                                            ui.label(
                                                                egui::RichText::new(&self.input_buffer[0..start])
                                                                    .font(egui::FontId::monospace(16.0))
                                                                    .color(egui::Color32::from_rgb(255, 255, 255))
                                                            );
                                                        }
                                                        
                                                        // Render selected part with bright background
                                                        if start < end {
                                                            ui.label(
                                                                egui::RichText::new(&self.input_buffer[start..end])
                                                                    .font(egui::FontId::monospace(16.0))
                                                                    .color(egui::Color32::from_rgb(255, 255, 255))
                                                                    .background_color(egui::Color32::from_rgb(0, 120, 255)) // Bright blue
                                                            );
                                                        }
                                                        
                                                        // Render unselected part after selection
                                                        if end < self.input_buffer.len() {
                                                            ui.label(
                                                                egui::RichText::new(&self.input_buffer[end..])
                                                                    .font(egui::FontId::monospace(16.0))
                                                                    .color(egui::Color32::from_rgb(255, 255, 255))
                                                            );
                                                        }
                                                        
                                                        // Add cursor if it's at the end
                                                        if self.show_cursor && self.cursor_pos >= self.input_buffer.len() {
                                                            ui.label(
                                                                egui::RichText::new("█")
                                                                    .font(egui::FontId::monospace(16.0))
                                                                    .color(egui::Color32::from_rgb(255, 255, 255))
                                                            );
                                                        }
                                                    } else {
                                                        // No selection - render normally with cursor
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
                                                                .color(egui::Color32::from_rgb(255, 255, 255))
                                                        );
                                                    }
                                                });
                                            });

                                            // Show autocomplete suggestions
                                            if self.show_autocomplete && !self.autocomplete_suggestions.is_empty() {
                                                ui.add_space(10.0);
                                                ui.separator();
                                                ui.add_space(5.0);

                                                // Show suggestions in a grid-like layout
                                                let suggestions_per_row = 4;
                                                let mut current_row = Vec::new();

                                                for (i, suggestion) in self.autocomplete_suggestions.iter().enumerate() {
                                                    let color = if i == self.autocomplete_index as usize {
                                                        egui::Color32::from_rgb(255, 255, 100) // Yellow highlight for selected
                                                    } else {
                                                        egui::Color32::from_rgb(150, 150, 150) // Gray for others
                                                    };

                                                    current_row.push((suggestion.clone(), color));

                                                    // Start new row or show current row
                                                    if current_row.len() == suggestions_per_row || i == self.autocomplete_suggestions.len() - 1 {
                                                        ui.horizontal(|ui| {
                                                            for (sugg, col) in &current_row {
                                                                ui.label(
                                                                    egui::RichText::new(sugg)
                                                                        .font(egui::FontId::monospace(14.0))
                                                                        .color(*col)
                                                                );
                                                                ui.add_space(15.0); // Space between suggestions
                                                            }
                                                        });
                                                        current_row.clear();
                                                    }
                                                }

                                                ui.add_space(5.0);
                                                ui.label(
                                                    egui::RichText::new(format!("{} suggestions (Tab to cycle, Enter to select)", self.autocomplete_suggestions.len()))
                                                        .font(egui::FontId::monospace(12.0))
                                                        .color(egui::Color32::from_rgb(100, 100, 100))
                                                );
                                            }
                                        }
                                    }
                                });
                            });

                        // Status bar (simplified)
                        ui.separator();
                        ui.horizontal(|ui| {
                            let fuzzy_status = if self.fuzzy_enabled { "ON" } else { "OFF" };
                            let status_text = if self.show_autocomplete && !self.autocomplete_suggestions.is_empty() {
                                format!("{} | Fuzzy: {} | Ctrl+C/X/V: clipboard | Ctrl+A: select all | Tab: cycle ({}/{}) | Ctrl+Space: toggle | Ctrl+F: fuzzy",
                                    self.current_dir,
                                    fuzzy_status,
                                    self.autocomplete_index + 1,
                                    self.autocomplete_suggestions.len())
                            } else {
                                format!("{} | Fuzzy: {} | Ctrl+C/X/V: clipboard | Ctrl+A: select all | Ctrl+Space: show suggestions | Ctrl+F: fuzzy",
                                    self.current_dir,
                                    fuzzy_status)
                            };
                            ui.small(status_text);
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
// Development milestone: Security features added
// Development milestone: Async processing with Tokio
// Development milestone: Error handling and fallbacks
// Development milestone: Modular architecture
// Development milestone: Cross-platform compatibility
// Development milestone: README documentation
// Development milestone: Project presentation and docs
// Development milestone: Final features and polish
