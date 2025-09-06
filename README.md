# snapshell (ss)

Minimal and snappy shell command generator with LLM/AI. 

Alternative to GitHub Copilot `ghcs`, snapshell quickly generates shell commands using your preferred LLM/AI via OpenRouter.

Install
-------

Install via crates.io (recommended):

```bash
curl https://sh.rustup.rs -sSf | sh

cargo install snapshell
```

Set up PATH and symlink (optional):

```bash
# The binary is installed to ~/.cargo/bin by default; make sure it's on your PATH:
export PATH="$HOME/.cargo/bin:$PATH"

# Optionally create a global symlink so the command is `ss`:
ln -s "$HOME/.cargo/bin/snapshell" /usr/local/bin/ss
```

Build
-------

Build from source and symlink to `ss`:

```bash
cargo build --release
# Use sudo if /usr/local/bin requires elevated permissions
sudo ln -s "$(pwd)/target/release/snapshell" /usr/local/bin/ss
```

OpenRouter configuration
------------------------

Before using snapshell with LLM features, configure OpenRouter:

- Export your API key (and optional model) for the session:

```bash
export SNAPSHELL_OPENROUTER_API_KEY="your_openrouter_api_key"
export SNAPSHELL_OPENROUTER_MODEL="openai/gpt-oss-120b"  # optional override, e.g. meta-llama/llama-3.3-8b-instruct:free
```

- Or create a `.env` file based on `.env.example` and load it with your shell or a tool like `direnv`:

```bash
cp .env.example .env
# edit .env and add your key and optional model:
# SNAPSHELL_OPENROUTER_API_KEY=your_openrouter_api_key
# SNAPSHELL_OPENROUTER_MODEL=openai/gpt-oss-120b
export $(cat .env | xargs)
```


Permanent setup (bash / zsh)
---------------------------

To make the key (and optional model) permanent, add the exports to your shell startup file.

For bash (`~/.bashrc` or `~/.profile`):

```bash
echo 'export SNAPSHELL_OPENROUTER_API_KEY="your_openrouter_api_key"' >> ~/.bashrc
echo 'export SNAPSHELL_OPENROUTER_MODEL="openai/gpt-oss-120b"' >> ~/.bashrc
# or
echo 'export SNAPSHELL_OPENROUTER_API_KEY="your_openrouter_api_key"' >> ~/.profile
echo 'export SNAPSHELL_OPENROUTER_MODEL="openai/gpt-oss-120b"' >> ~/.profile
```

For zsh (`~/.zshrc` or `~/.zprofile`):

```bash
echo 'export SNAPSHELL_OPENROUTER_API_KEY="your_openrouter_api_key"' >> ~/.zshrc
echo 'export SNAPSHELL_OPENROUTER_MODEL="openai/gpt-oss-120b"' >> ~/.zshrc
# or
echo 'export SNAPSHELL_OPENROUTER_API_KEY="your_openrouter_api_key"' >> ~/.zprofile
echo 'export SNAPSHELL_OPENROUTER_MODEL="openai/gpt-oss-120b"' >> ~/.zprofile
```

After editing, reload your shell or source the file:

```bash
source ~/.bashrc   # or source ~/.zshrc
```


Quick usage
-----------

- `ss 'describe what shell command you want'`
	- Generate a single-line shell command, print it, copy to macOS clipboard, and save to history.
- `ss -a 'chat with the model'`
	- Enter interactive chat mode; you can continue asking follow-ups. Type `/exit` or empty line to quit.
- `ss -r 2 'use reasoning level 2'`
	- Attach a reasoning hint to the model.
- `ss -m 'provider/model' 'ask'`
	- Override the model (use provider-specific model strings like `groq/...` or `cerebras/...`).
- `ss -L 'ask'`
	- Allow multiline script output instead of forcing one-liner.
- `ss -H`
	- Print saved history entries.

Flags & examples
-----------------

- Default single-line mode (default behavior):

```bash
ss "install openvino and show the command to quantize a tensorflow model"
```

- Force multiline output (for scripts):

```bash
ss -L "generate a bash script to backup ~/projects to /tmp/backup"
```

- Interactive chat mode (follow-ups):

```bash
ss -a "how to list modified rust files since yesterday?"
# After response, type follow-up questions at the `>` prompt
```

- Use a low-latency free model:

```bash
ss -m "meta-llama/llama-3.3-8b-instruct:free" "list files modified today"
```

- Override the default system instruction (applies to both modes unless more specific):

```bash
ss -s "You are an expert devops assistant. Output only shell commands." "describe what you want"
```

- Override single-line or multiline system instruction explicitly:

```bash
ss --system-single "Single-line-only instruction" "do X"
ss --system-multiline "Multiline-allowed instruction" -L "do Y"
```

- View history:

```bash
ss -H
```

Reasoning
---------

snapshell supports an optional lightweight "reasoning" hint (OpenAI-style `effort`) you can request from the model.

- `-r, --reasoning <low|medium|high>` — set the reasoning effort. Default: `low`.
- `-S, --show-reasoning` — when set, the model may append a trailing JSON object containing the model's short reasoning, printed on the line after the command as:

```json
{"reasoning": "short one-sentence reason here"}
```

Notes:
- Reasoning is not printed by default; only enable it with `-S` when you want an explanation.
- The reasoning line is not copied to the clipboard and is not saved to history; only the generated command is copied/saved.
- Example:

```bash
ss -r high -S "why can't I install TensorRT on macOS?"
# output:
# (NOT ABLE TO ANSWER): TensorRT requires NVIDIA GPUs and is not available on macOS.
# {"reasoning": "TensorRT depends on NVIDIA GPU drivers not present on macOS"}
```


Environment variables
---------------------

- `SNAPSHELL_OPENROUTER_API_KEY` — API key for OpenRouter (required to call remote LLM).
- `SNAPSHELL_SYSTEM` — generic system instruction override.
- `SNAPSHELL_SYSTEM_SINGLE` — override for single-line mode.
- `SNAPSHELL_SYSTEM_MULTILINE` — override for multiline mode.

See `.env.example` for a sample env file.

OpenRouter integration
----------------------

This tool is integrated with OpenRouter. Provide your OpenRouter API key via the environment variable `SNAPSHELL_OPENROUTER_API_KEY`.

You can control the model used in two ways (priority order):

1. CLI: pass `-m 'provider/model'` to `ss`.
2. Environment: set `SNAPSHELL_OPENROUTER_MODEL` (for example `openai/gpt-oss-120b` or `groq/fast-model`).

If neither is set, snapshell falls back to the built-in default `openai/gpt-oss-120b`.

For the instant result, lowest-latency replies providers recommended are [Groq](https://openrouter.ai/provider/groq) and [Cerebras](https://openrouter.ai/provider/cerebras) when available, this provider use specialized inference hardware that can significantly speed up response times with 1K tokens/second. 

You can enforce this provider in Open Router: Settings > Account > Allowed Providers > Select a provider, you can select both [Groq](https://openrouter.ai/provider/groq) and [Cerebras](https://openrouter.ai/provider/cerebras). Also tick the 'Always enforce' checkbox.

History
-------

History is saved as `history.jsonl` in your OS data dir and contains timestamp, prompt, and generated command. Use `ss -H` to view.

Notes
-----

- Minimal, fast, designed to return only shell commands by default.
- If the model returns extra text, use `-s`/`--system-single`/`--system-multiline` to tighten instructions.
