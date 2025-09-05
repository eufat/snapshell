use anyhow::Result;
use clap::{Arg, ArgAction, Command};
use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct HistoryEntry {
    timestamp: String,
    prompt: String,
    command: String,
}

#[derive(Deserialize)]
struct OpenRouterChoiceMessage {
    content: String,
    // OpenRouter may include a reasoning object on the message
    reasoning: Option<JsonValue>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterChoiceMessage,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("snapshell")
        .about("Snappy shell command generation (minimal)")
        .arg(Arg::new("input").help("Command instruction or chat text").index(1).num_args(1).required(false))
        .arg(
            Arg::new("history")
                .short('H')
                .long("history")
                .help("Show history of prompts and generated commands")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("all")
                .short('a')
                .long("ask")
                .help("Interactive LLM chat mode (prints conversation)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("reasoning")
                .short('r')
                .long("reasoning")
                .help("Reasoning effort: low, medium, or high (default: low)")
                .num_args(1),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .help("Model to use (defaults to openai/gpt-oss-20b)")
                .num_args(1),
        )
        .arg(
            Arg::new("multiline")
                .short('L')
                .long("multiline")
                .help("Allow multiline/multi-line shell script output instead of forcing a single-line command")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("system")
                .short('s')
                .long("system")
                .help("Custom system instruction (overrides defaults). Can be used for both single- and multiline modes unless specific flags are provided.")
                .num_args(1),
        )
        .arg(
            Arg::new("system-single")
                .long("system-single")
                .help("Custom system instruction for single-line mode")
                .num_args(1),
        )
        .arg(
            Arg::new("system-multiline")
                .long("system-multiline")
                .help("Custom system instruction for multiline mode")
                .num_args(1),
        )
        .arg(
            Arg::new("show-reasoning")
                .short('S')
                .long("show-reasoning")
                .help("Include model reasoning in output as a trailing JSON object {\"reasoning\": \"...\"}")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    let prompt = matches
        .get_one::<String>("input")
        .cloned()
        .or_else(|| std::env::args().nth(1));

    let interactive = matches.get_flag("all");
    let show_history = matches.get_flag("history");

    if show_history {
        if let Err(e) = print_history() {
            eprintln!("Failed to read history: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    let prompt = match prompt {
        Some(p) => p,
        None => {
            eprintln!("Usage: ss 'command instructions'  (or ss -a 'ask something')");
            std::process::exit(1);
        }
    };

    let model = matches
        .get_one::<String>("model")
        .map(|s| s.as_str())
        .unwrap_or("openai/gpt-oss-20b");

    // Read SNAPSHELL_OPENROUTER_API_KEY from env or config (intentionally not backwards-compatible)
    let api_key = std::env::var("SNAPSHELL_OPENROUTER_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        eprintln!("Set SNAPSHELL_OPENROUTER_API_KEY env var for OpenRouter integration.");
    }

    // Build request payload with support for configurable system instructions.
    let allow_multiline = matches.get_flag("multiline");

    // Read optional custom system instructions from CLI or env vars.
    let cli_system = matches.get_one::<String>("system").map(|s| s.as_str());
    let cli_system_single = matches.get_one::<String>("system-single").map(|s| s.as_str());
    let cli_system_multi = matches.get_one::<String>("system-multiline").map(|s| s.as_str());

    let env_system = std::env::var("SNAPSHELL_SYSTEM").ok();
    let env_system_single = std::env::var("SNAPSHELL_SYSTEM_SINGLE").ok();
    let env_system_multi = std::env::var("SNAPSHELL_SYSTEM_MULTILINE").ok();

    // Prepare messages vector. If not interactive, choose a system instruction using priority:
    // CLI specific > CLI generic > ENV specific > ENV generic > built-in default.
    let mut messages = Vec::new();
    if !interactive {
    let default_single = "You are a strict shell command generator. OUTPUT ONLY shell commands or shell syntax in plain text with no explanations, no commentary, and no additional prose. DO NOT output any markdown, code fences, backticks, or formatting of any kind. The entire response MUST be a single-line shell command with no extra text. Never add numbering, bullets, examples, or any text before or after the command. If you do NOT know the correct command, respond exactly with the following format and nothing else: (NOT ABLE TO ANSWER): <one-sentence reason> — the reason should be a single short sentence explaining why the command cannot be provided. Always respond only with the shell command(s) or the one-line failure phrase in the format above.";
    let default_multi = "You are a strict shell command generator. OUTPUT ONLY shell commands or shell syntax in plain text with no explanations, no commentary, and no additional prose. DO NOT output any markdown, code fences, backticks, or formatting of any kind. Multi-line shell scripts are allowed when necessary. Never add numbering, bullets, examples, or any text before or after the command. If you do NOT know the correct command, respond exactly with the following format and nothing else: (NOT ABLE TO ANSWER): <one-sentence reason> — the reason should be a single short sentence explaining why the command cannot be provided. Always respond only with the shell command(s) or the one-line failure phrase in the format above.";

    let mut sys = if let Some(s) = cli_system { s.to_string() }
        else if allow_multiline {
            if let Some(s) = cli_system_multi { s.to_string() }
            else if let Some(s) = env_system_multi { s }
            else if let Some(s) = env_system.clone() { s }
            else { default_multi.to_string() }
        } else {
            if let Some(s) = cli_system_single { s.to_string() }
            else if let Some(s) = env_system_single.clone() { s }
            else if let Some(s) = env_system.clone() { s }
            else { default_single.to_string() }
        };

    // Append detected environment note so the model tailors commands to the user's OS/distro
    let env_note = format!(" Target environment: {}. Ensure generated commands are compatible with this environment.", detect_environment());
    sys.push_str(&env_note);

    messages.push(serde_json::json!({"role": "system", "content": sys}));
    }

    // Determine reasoning settings (OpenAI-style 'effort')
    let effort = matches
        .get_one::<String>("reasoning")
        .map(|s| s.as_str())
        .unwrap_or("low");
    let show_reasoning = matches.get_flag("show-reasoning");

    // Append the initial user prompt
    messages.push(serde_json::json!({"role": "user", "content": prompt}));

    if interactive {
        // Interactive loop: keep conversation messages and prompt user after each model response.
        println!("Entering interactive chat mode. Type '/exit' or empty line to quit.");
        // messages already contains any system instructions (none in interactive) and the first user prompt
        loop {
            // Include top-level reasoning object following OpenRouter's API (e.g. { "reasoning": { "effort": "high" } })
            let body = serde_json::json!({"model": model, "messages": messages, "reasoning": {"effort": effort}});
            let cli_output = query_openrouter(&api_key, &body).await.unwrap_or_else(|e| {
                eprintln!("LLM request failed: {}", e);
                std::process::exit(1);
            });

            let response = cli_output
                .choices
                .get(0)
                .map(|c| c.message.content.clone())
                .unwrap_or_default();

            // Print assistant response
            println!("{}", response.trim());

            // If show_reasoning is requested, the model may include a trailing reasoning field; print nothing here — interactive mode shows full assistant response.

            // Append assistant message to conversation
            messages.push(serde_json::json!({"role": "assistant", "content": response}));

            // Prompt for next user input
            print!("> ");
            let _ = io::stdout().flush();
            let mut line = String::new();
            if let Err(_) = io::stdin().read_line(&mut line) {
                break;
            }
            let line = line.trim().to_string();
            if line.is_empty() || line == "/exit" {
                break;
            }
            // add user message and continue loop
            messages.push(serde_json::json!({"role": "user", "content": line}));
        }
    } else {
        // Include top-level reasoning object following OpenRouter's API
        let body = serde_json::json!({"model": model, "messages": messages, "reasoning": {"effort": effort}});

        let cli_output = query_openrouter(&api_key, &body).await.unwrap_or_else(|e| {
            eprintln!("LLM request failed: {}", e);
            std::process::exit(1);
        });
        // The API returns choices[].message.content and may include choices[].message.reasoning
        let choice = cli_output.choices.get(0);
        let command = choice.map(|c| c.message.content.clone()).unwrap_or_default();

        // Grab reasoning from the parsed response if available
        let reasoning_json = if show_reasoning {
            choice
                .and_then(|c| c.message.reasoning.clone())
        } else {
            None
        };

        let out = command.trim().to_string();

        // Minimal: print only the command (out was derived above)
        if is_not_able_response(&out) {
            // Uncopyable message: print but do not copy to clipboard or save to history
            println!("{}", out);
        } else {
            println!("{}", out);

            // Copy to clipboard on macOS
            #[cfg(target_os = "macos")]
            {
                if let Ok(child) = std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn() {
                    if let Some(mut stdin) = child.stdin {
                        let _ = stdin.write_all(out.as_bytes());
                    }
                }
            }

            // Save history
            save_history(&prompt, &out)?;
        }

        if let Some(js_val) = reasoning_json {
            // Normalize the reasoning output to the canonical form: {"reasoning": "..."}
            // If the model returned a string, wrap it. If it returned an object that includes
            // a `reasoning` key, prefer that. Otherwise stringify the object and wrap it.
            let final_obj = if js_val.is_string() {
                let s = js_val.as_str().unwrap_or_default();
                serde_json::json!({"reasoning": s})
            } else if js_val.is_object() {
                // If it already contains a `reasoning` key, use as-is
                if js_val.get("reasoning").is_some() {
                    js_val
                } else {
                    // Fallback: stringify the object and place under `reasoning`
                    let s = serde_json::to_string(&js_val).unwrap_or_else(|_| js_val.to_string());
                    serde_json::json!({"reasoning": s})
                }
            } else {
                // Other types (numbers, arrays, etc.) - stringify and wrap
                let s = serde_json::to_string(&js_val).unwrap_or_else(|_| js_val.to_string());
                serde_json::json!({"reasoning": s})
            };

            // Print compact single-line JSON to match README examples
            if let Ok(s) = serde_json::to_string(&final_obj) {
                println!("{}", s);
            } else {
                println!("{}", final_obj);
            }
        }
    }

    Ok(())
}

async fn query_openrouter(api_key: &str, body: &serde_json::Value) -> Result<OpenRouterResponse> {
    let client = reqwest::Client::new();
    let mut req = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .json(body);

    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = req.send().await?.error_for_status()?;
    let out = resp.json::<OpenRouterResponse>().await?;
    Ok(out)
}

fn history_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "snapshell", "snapshell").map(|d| d.data_local_dir().join("history.jsonl"))
}

fn save_history(prompt: &str, command: &str) -> Result<()> {
    if let Some(path) = history_path() {
        if let Some(dir) = path.parent() {
            create_dir_all(dir)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        let entry = HistoryEntry {
            timestamp: Utc::now().to_rfc3339(),
            prompt: prompt.to_string(),
            command: command.to_string(),
        };
        let line = serde_json::to_string(&entry)? + "\n";
        file.write_all(line.as_bytes())?;
    }
    Ok(())
}

fn print_history() -> Result<()> {
    if let Some(path) = history_path() {
        if !path.exists() {
            println!("no history");
            return Ok(());
        }
        let mut s = String::new();
        let mut f = std::fs::File::open(&path)?;
        f.read_to_string(&mut s)?;
        for line in s.lines() {
            if let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) {
                println!("{} -> {}\n  {}", entry.timestamp, entry.prompt, entry.command);
            }
        }
    } else {
        println!("no history");
    }
    Ok(())
}

fn detect_environment() -> String {
    // macOS
    if cfg!(target_os = "macos") {
        return "macos".to_string();
    }

    // Windows
    if cfg!(target_os = "windows") {
        return "windows".to_string();
    }

    // Try to read /etc/os-release for Linux distros
    if cfg!(target_os = "linux") {
        if let Ok(s) = std::fs::read_to_string("/etc/os-release") {
            let s_l = s.to_lowercase();
            if s_l.contains("debian") || s_l.contains("ubuntu") {
                return "linux (debian/ubuntu)".to_string();
            }
            if s_l.contains("fedora") {
                return "linux (fedora)".to_string();
            }
            if s_l.contains("arch") {
                return "linux (arch)".to_string();
            }
            // fallback for generic linux
            return "linux".to_string();
        }
        return "linux".to_string();
    }

    // Unknown/fallback
    "unknown".to_string()
}

fn is_not_able_response(s: &str) -> bool {
    // Expect format: (NOT ABLE TO ANSWER): <reason>
    let s = s.trim();
    if s.len() < 22 {
        return false;
    }
    // Case-insensitive check for the prefix
    let lower = s.to_lowercase();
    lower.starts_with("(not able to answer):")
}
