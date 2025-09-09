use reqwest;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tokio::sync::mpsc;
use std::env;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// OpenRouter API endpoint
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Get the OpenRouter API key from environment variable
fn get_openrouter_api_key() -> Result<String, String> {
    env::var("OPENROUTER_API_KEY")
        .map_err(|_| "OPENROUTER_API_KEY environment variable not set. Please set it with: export OPENROUTER_API_KEY='your_api_key_here'".to_string())
}

#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Clone)]
struct CacheEntry {
    command: String,
    timestamp: SystemTime,
}

pub struct AIAssistant {
    client: reqwest::Client,
    pub sender: mpsc::UnboundedSender<String>,
    pub receiver: mpsc::UnboundedReceiver<String>,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    local_commands: HashMap<String, String>,
}

impl AIAssistant {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        // Configure HTTP client for maximum speed
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5)) // Even faster timeout
            .pool_max_idle_per_host(20) // More connection pooling
            .pool_idle_timeout(Duration::from_secs(60)) // Longer keep-alive
            .tcp_keepalive(Duration::from_secs(60)) // TCP keepalive
            .build()
            .expect("Failed to build HTTP client");

        // Initialize local command mappings for instant responses
        let mut local_commands = HashMap::new();

        // Most common commands - instant responses, no API call
        local_commands.insert("list files".to_string(), "ls".to_string());
        local_commands.insert("show files".to_string(), "ls".to_string());
        local_commands.insert("list directory".to_string(), "ls".to_string());
        local_commands.insert("show directory".to_string(), "ls".to_string());
        local_commands.insert("what files are here".to_string(), "ls".to_string());
        local_commands.insert("see files".to_string(), "ls".to_string());

        local_commands.insert("list all files".to_string(), "ls -la".to_string());
        local_commands.insert("show all files".to_string(), "ls -la".to_string());
        local_commands.insert("list hidden files".to_string(), "ls -la".to_string());
        local_commands.insert("show hidden files".to_string(), "ls -la".to_string());

        local_commands.insert("go up".to_string(), "cd ..".to_string());
        local_commands.insert("go back".to_string(), "cd ..".to_string());
        local_commands.insert("go to parent".to_string(), "cd ..".to_string());
        local_commands.insert("up one level".to_string(), "cd ..".to_string());

        local_commands.insert("go home".to_string(), "cd ~".to_string());
        local_commands.insert("go to home".to_string(), "cd ~".to_string());
        local_commands.insert("home directory".to_string(), "cd ~".to_string());

        local_commands.insert("show current directory".to_string(), "pwd".to_string());
        local_commands.insert("where am i".to_string(), "pwd".to_string());
        local_commands.insert("current location".to_string(), "pwd".to_string());
        local_commands.insert("print working directory".to_string(), "pwd".to_string());

        local_commands.insert("clear screen".to_string(), "clear".to_string());
        local_commands.insert("clear terminal".to_string(), "clear".to_string());
        local_commands.insert("clean screen".to_string(), "clear".to_string());

        local_commands.insert("show date".to_string(), "date".to_string());
        local_commands.insert("what time is it".to_string(), "date".to_string());
        local_commands.insert("current time".to_string(), "date".to_string());

        local_commands.insert("show calendar".to_string(), "cal".to_string());
        local_commands.insert("calendar".to_string(), "cal".to_string());
        local_commands.insert("show month".to_string(), "cal".to_string());

        // Common remove operations with spaces in filenames
        local_commands.insert("remove folder".to_string(), "rm -r".to_string());
        local_commands.insert("delete folder".to_string(), "rm -r".to_string());
        local_commands.insert("remove directory".to_string(), "rm -r".to_string());
        local_commands.insert("delete directory".to_string(), "rm -r".to_string());

        Self {
            client,
            sender,
            receiver,
            cache: Arc::new(Mutex::new(HashMap::new())),
            local_commands,
        }
    }

    /// Check cache for existing response (5-minute TTL)
    fn get_cached_response(&self, input: &str) -> Option<String> {
        if let Ok(cache) = self.cache.lock() {
            if let Some(entry) = cache.get(input) {
                if entry.timestamp.elapsed().unwrap_or(Duration::from_secs(0)) < Duration::from_secs(300) {
                    return Some(entry.command.clone());
                }
            }
        }
        None
    }

    /// Store response in cache
    fn cache_response(&self, input: &str, command: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(input.to_string(), CacheEntry {
                command: command.to_string(),
                timestamp: SystemTime::now(),
            });
            // Keep cache size manageable (max 100 entries)
            if cache.len() > 100 {
                // Remove oldest entries (simple FIFO)
                let keys_to_remove: Vec<String> = cache.keys()
                    .take(cache.len() - 100)
                    .cloned()
                    .collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
        }
    }

    pub fn is_natural_language(input: &str) -> bool {
        let input = input.trim().to_lowercase();

        // Skip if it's already a command (starts with common command words)
        if input.starts_with("mkdir") || input.starts_with("ls") || input.starts_with("cd")
           || input.starts_with("rm") || input.starts_with("cp") || input.starts_with("mv")
           || input.starts_with("git") || input.starts_with("curl") || input.starts_with("wget")
           || input.starts_with("sudo") || input.starts_with("chmod") || input.starts_with("grep")
           || input.starts_with("open") {
            return false;
        }

        // Check if input is gibberish (contains only repeated characters or no vowels)
        if Self::is_gibberish(&input) {
            return false;
        }

        // Detect natural language patterns
        let natural_indicators = [
            "create a", "make a", "delete", "remove", "list", "show me", "find",
            "search for", "copy", "move", "download", "install", "update",
            "how to", "i want to", "can you", "please", "help me",
            "open this", "open file", "open in", "launch", "start",
            "open folder", "open current", "open here", "open directory",
            "cursor", "vscode", "editor", "ide"
        ];

        natural_indicators.iter().any(|&indicator| input.contains(indicator))
    }

    /// Check if input appears to be gibberish or nonsensical
    pub fn is_gibberish(input: &str) -> bool {
        let input = input.trim().to_lowercase();

        // Too short - likely not meaningful
        if input.len() < 2 {
            return true;
        }

        // Allow inputs that contain meaningful IDE/editor related words
        let meaningful_words = ["open", "cursor", "vscode", "editor", "ide", "folder", "directory", "file", "this", "here", "current"];
        if meaningful_words.iter().any(|&word| input.contains(word)) {
            return false;
        }

        // Check for repeated characters (like "aaaaa", "sdasdasdasdas")
        let chars: Vec<char> = input.chars().collect();
        if chars.len() >= 4 {
            let mut repeated_count = 1;
            for i in 1..chars.len() {
                if chars[i] == chars[i-1] {
                    repeated_count += 1;
                    if repeated_count >= 4 {
                        return true;
                    }
                } else {
                    repeated_count = 1;
                }
            }
        }

        // Check if input has no alphanumeric characters
        if !input.chars().any(|c| c.is_alphanumeric()) {
            return true;
        }

        // Check for patterns that suggest gibberish (like alternating same characters)
        if input.len() >= 6 {
            let mut alternating_pattern = true;
            for i in 2..input.len() {
                if input.chars().nth(i) != input.chars().nth(i-2) {
                    alternating_pattern = false;
                    break;
                }
            }
            if alternating_pattern {
                return true;
            }
        }

        // Check for incoherent phrases (like "how hello", "what is", etc.)
        let incoherent_patterns = [
            "how hello", "hello how", "what hello", "hello what",
            "why hello", "hello why", "when hello", "hello when",
            "where hello", "hello where", "who hello", "hello who",
            "how what", "what how", "why what", "what why",
            "how are", "what are", "why are", "when are", "where are", "who are",
            "hello world", "world hello", "test hello", "hello test"
        ];

        if incoherent_patterns.iter().any(|&pattern| input.contains(pattern)) {
            return true;
        }

        // Check for inputs that are just question words without context
        let question_words = ["how", "what", "why", "when", "where", "who", "which"];
        let words: Vec<&str> = input.split_whitespace().collect();

        if words.len() <= 3 {
            // If it's just 1-3 words and contains question words without meaningful context
            let has_question = words.iter().any(|&word| question_words.contains(&word));
            let has_meaningful = words.iter().any(|&word|
                ["create", "make", "delete", "remove", "list", "show", "find", "search",
                 "copy", "move", "download", "install", "update", "open", "close", "start", "stop"].contains(&word)
            );

            if has_question && !has_meaningful {
                return true;
            }
        }

        false
    }

    /// Quick validation that a suggested command looks executable on this system.
    fn looks_like_valid_command(command: &str) -> bool {
        let trimmed = command.trim();
        if trimmed.is_empty() { return false; }

        // Remove code fences if present
        let cleaned = trimmed
            .trim_start_matches("```bash")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // First token is the executable/builtin
        let mut parts = cleaned.split_whitespace();
        let first = match parts.next() { Some(f) => f, None => return false };

        // Allow a few known non-PATH tools/builtins
        let allowlist = ["cd", "cursor", "code", "xdg-open"];
        if allowlist.contains(&first) { return true; }

        // Reject if it starts like a sentence or only punctuation/letters like "hello"
        if first.starts_with('-') { return false; }

        // If path-like, ensure it's executable
        if first.contains('/') { return Self::is_executable_path(first); }

        // Check PATH for executable
        if let Ok(path) = env::var("PATH") {
            for dir in path.split(':') {
                let p = std::path::Path::new(dir).join(first);
                if p.is_file() {
                    #[cfg(unix)]
                    {
                        if let Ok(meta) = std::fs::metadata(&p) {
                            if meta.permissions().mode() & 0o111 != 0 { return true; }
                        }
                    }
                    #[cfg(not(unix))]
                    { return true; }
                }
            }
        }
        false
    }

    #[allow(dead_code)]
    fn is_executable_path(path: &str) -> bool {
        let p = std::path::Path::new(path);
        if !p.is_file() { return false; }
        #[cfg(unix)]
        {
            if let Ok(meta) = std::fs::metadata(p) {
                return meta.permissions().mode() & 0o111 != 0;
            }
            false
        }
        #[cfg(not(unix))]
        { true }
    }

    /// Check if input matches a local command mapping (instant response)
    fn get_local_command(&self, input: &str) -> Option<String> {
        let input_lower = input.trim().to_lowercase();

        // Direct lookup
        if let Some(cmd) = self.local_commands.get(&input_lower) {
            return Some(cmd.to_string());
        }

        // Fuzzy matching for common variations
        for (key, cmd) in &self.local_commands {
            if input_lower.contains(key) || key.contains(&input_lower) {
                return Some(cmd.to_string());
            }
        }

        None
    }

    /// Check for ultra-fast local responses (no async needed)
    pub fn get_instant_command(input: &str) -> Option<String> {
        let input_lower = input.trim().to_lowercase();

        // Most common commands that should be instant
        match input_lower.as_str() {
            "list files" | "show files" | "ls" | "dir" => Some("ls".to_string()),
            "list all files" | "show all files" | "ls -la" | "dir /a" => Some("ls -la".to_string()),
            "go up" | "cd .." | "up" | "back" => Some("cd ..".to_string()),
            "go home" | "cd ~" | "home" => Some("cd ~".to_string()),
            "where am i" | "pwd" | "current directory" => Some("pwd".to_string()),
            "clear" | "cls" | "clear screen" => Some("clear".to_string()),
            "date" | "time" | "what time is it" => Some("date".to_string()),
            "calendar" | "cal" | "show calendar" => Some("cal".to_string()),
            "help" | "commands" | "?" => Some("help".to_string()),
            _ => None,
        }
    }

    pub async fn generate_command(&self, natural_input: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // First check if input is gibberish
        if Self::is_gibberish(natural_input) {
            return Err("I don't understand that input. Please provide a clear command or natural language request.".into());
        }

        // Check local commands first (INSTANT responses)
        if let Some(local_cmd) = self.get_local_command(natural_input) {
            return Ok(local_cmd);
        }

        // Check cache second
        if let Some(cached_command) = self.get_cached_response(natural_input) {
            return Ok(cached_command);
        }

        // Optimized shorter prompt for faster processing
        let prompt = format!(
            "Convert natural language to Linux command. Respond ONLY with command, no explanation.

Rules:
- Gibberish/nonsense → I_DONT_UNDERSTAND
- Questions without context → I_DONT_UNDERSTAND
- Valid requests → command only
- For filenames with spaces, use quotes: \"file name\" or 'file name'

Examples:
list files → ls
create folder test → mkdir test
remove file → rm file
delete folder → rm -r folder
remove my folder → rm -r \"my folder\"
delete test file → rm \"test file\"
delete old folder → rm -r \"old folder\"
open folder in editor → cursor .

Input: {}
Command:",
            natural_input
        );

        let request = OpenRouterRequest {
            model: "meta-llama/llama-3.2-3b-instruct:free".to_string(), // Using a good model for command generation
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: Some(20),
            temperature: Some(0.1), // Low temperature for consistent command generation
        };

        let api_key = get_openrouter_api_key().map_err(|e| e)?;
        let url = OPENROUTER_URL.to_string();

        // Ultra-fast timeout for instant feel
        let response = timeout(Duration::from_secs(3),
            self.client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
        ).await??;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or("failed to get response body".to_string());
            return Err(format!("API error: {} - {}", status, body).into());
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;

        // Extract first choice text
        let command = if let Some(choice) = openrouter_response.choices.first() {
            choice.message.content.trim().to_string()
        } else { String::new() };

        // Clean up the response - remove markdown formatting if present
        let command = command.trim_start_matches("```bash").trim_start_matches("```").trim_end_matches("```").trim().to_string();

        // Check if AI responded that it doesn't understand
        if command == "I_DONT_UNDERSTAND" {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        // Validate the response - make sure it's not the same as input
        if command == natural_input.trim() {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        // Basic validation - check if response looks like a command
        if command.is_empty() || command.len() > 200 || !command.chars().any(|c| c.is_alphanumeric()) {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        // Stronger validation: ensure first token is a known/builtin or executable in PATH
        if !Self::looks_like_valid_command(&command) {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        // Cache successful response
        self.cache_response(natural_input, &command);

        return Ok(command.to_string());
    }

    pub fn request_command_async(&self, input: String) {
        let sender = self.sender.clone();
        let client = self.client.clone();
        
        tokio::spawn(async move {
            match Self::generate_command_static(&client, &input).await {
                Ok(command) => {
                    let _ = sender.send(command);
                }
                Err(_) => {
                    // Silently fail - don't spam with errors
                }
            }
        });
    }

    pub async fn generate_command_static(client: &reqwest::Client, natural_input: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // First check if input is gibberish
        if Self::is_gibberish(natural_input) {
            return Err("I don't understand that input. Please provide a clear command or natural language request.".into());
        }

        let prompt = format!(
            "You are a Linux terminal command generator. Your task is to convert natural language requests into valid Linux commands.

IMPORTANT RULES:
- If the input is gibberish, nonsense, or doesn't make sense (like 'how hello', 'what is', 'hello world'), respond with exactly: \"I_DONT_UNDERSTAND\"
- If the input is not a valid command request, respond with exactly: \"I_DONT_UNDERSTAND\"
- If the input contains question words without meaningful command context, respond with exactly: \"I_DONT_UNDERSTAND\"
- Only respond with a valid Linux command if you can clearly understand the request
- Do NOT return the same input as output
- Do NOT try to interpret incoherent phrases as commands
- Respond ONLY with the command itself, no explanations, no markdown, no quotes

SPECIAL HANDLING FOR EDITORS/IDEs:
- \"open this folder in cursor\" → \"cursor .\"
- \"open current folder in vscode\" → \"code .\"
- \"open here in editor\" → \"cursor .\"
- \"open directory in ide\" → \"cursor .\"

SPECIAL HANDLING FOR GUI FILE MANAGERS:
- \"open this folder in gui\" → \"xdg-open .\"
- \"open current folder in file manager\" → \"xdg-open .\"
- \"show this folder in gui\" → \"xdg-open .\"
- \"open directory in file manager\" → \"xdg-open .\"

Examples:
- Input: \"list files\" → Output: \"ls\"
- Input: \"create folder test\" → Output: \"mkdir test\"
- Input: \"remove hello\" → Output: \"rm hello\"
- Input: \"remove hello folder\" → Output: \"rm -r hello\"
- Input: \"delete test file\" → Output: \"rm test\"
- Input: \"delete test directory\" → Output: \"rm -r test\"
- Input: \"remove my folder\" → Output: \"rm -r \"my folder\"\"
- Input: \"delete old file\" → Output: \"rm \"old file\"\"
- Input: \"remove SEM 3 folder\" → Output: \"rm -r \"SEM 3\"\"
- Input: \"delete my documents\" → Output: \"rm -r \"my documents\"\"
- Input: \"open this folder in cursor\" → Output: \"cursor .\"
- Input: \"open current directory in vscode\" → Output: \"code .\"
- Input: \"open this folder in gui\" → Output: \"xdg-open .\"
- Input: \"show folder in file manager\" → Output: \"xdg-open .\"
- Input: \"sdasdasdasdas\" → Output: \"I_DONT_UNDERSTAND\"
- Input: \"what is the meaning of life\" → Output: \"I_DONT_UNDERSTAND\"
- Input: \"how hello\" → Output: \"I_DONT_UNDERSTAND\"
- Input: \"hello world\" → Output: \"I_DONT_UNDERSTAND\"

Natural language: {}

Command:",
            natural_input
        );

        let request = OpenRouterRequest {
            model: "meta-llama/llama-3.2-3b-instruct:free".to_string(), // Using a good model for command generation
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: Some(20),
            temperature: Some(0.1), // Low temperature for consistent command generation
        };

        let api_key = get_openrouter_api_key().map_err(|e| e)?;
        let url = OPENROUTER_URL.to_string();

        // Reduced timeout for better UX
        let response = timeout(Duration::from_secs(3),
            client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
        ).await??;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or("failed to get response body".to_string());
            return Err(format!("API error: {} - {}", status, body).into());
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;

        // Extract first choice text
        let mut command = String::new();
        if let Some(choice) = openrouter_response.choices.first() {
            command = choice.message.content.trim().to_string();
        }
        // Clean up the response - remove markdown formatting if present
        let command = command.trim_start_matches("```bash").trim_start_matches("```").trim_end_matches("```").trim();

        // Check if AI responded that it doesn't understand
        if command == "I_DONT_UNDERSTAND" {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        // Validate the response - make sure it's not the same as input
        if command == natural_input.trim() {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        // Basic validation - check if response looks like a command
        if command.is_empty() || command.len() > 200 || !command.chars().any(|c| c.is_alphanumeric()) {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        // Stronger validation: ensure first token is a known/builtin or executable in PATH
        if !Self::looks_like_valid_command(&command) {
            return Err("I don't understand that request. Please try rephrasing your command.".into());
        }

        return Ok(command.to_string());
    }
}
