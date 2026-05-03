# fabrica

A terminal-based coding agent built in Rust.

## Features

- **Interactive TUI** — full terminal UI with scrollable conversation log, streaming responses, and an in-app model picker
- **Multi-provider support** — switch between Google Gemini, Anthropic Claude, and OpenAI models on the fly
- **Built-in tools** — the agent can read/write/edit files and run bash commands autonomously
- **Agentic loop** — the model plans and executes multi-step tasks using tool calls until the job is done

## Installation

### From crates.io

```bash
cargo install fabrica-cli
```

### From source

```bash
git clone https://github.com/Endi1/fabrica.git
cd fabrica
cargo install --path .
```

## Configuration

Set the API key for the provider you want to use as an environment variable:

| Provider          | Environment Variable |
|-------------------|----------------------|
| Google Gemini API | `GEMINI_KEY`         |
| Google Vertex AI  | `GCP_PROJECT`        |
| Anthropic Claude  | `ANTHROPIC_KEY`      |
| OpenAI            | `OPENAI_KEY`         |

## Supported Models

### Google Gemini API
- `gemini-2.5-flash`
- `gemini-3.1-pro`
- `gemini-3-flash`
- `gemini-3.1-flash-lite`

### Google Vertex AI
- `gemini-2.5-flash`
- `gemini-3.1-pro`
- `gemini-3-flash`
- `gemini-3.1-flash-lite`

### Anthropic Claude
- `claude-sonnet-4-5`
- `claude-opus-4-6`
- `claude-opus-4-7` *(default)*

### OpenAI
- `gpt-5.3-codex`
- `gpt-5.4`
- `gpt-5.4-mini`
- `gpt-5.4-nano`
- `gpt-5.5`

## Usage

Run `fabrica` in any project directory:

```bash
fabrica
```

### ACP Server Mode

fabrica can also run as a headless [Agent Client Protocol](https://agentclientprotocol.com) server, exposing the agent over JSON-RPC 2.0 on stdio. This allows any ACP-compatible client (IDEs, CLI tools, etc.) to drive fabrica programmatically.

```bash
fabrica serve
```


### Commands

| Command  | Description                                      |
|----------|--------------------------------------------------|
| `/model` | Open the model picker to switch providers/models |
| `/exit`  | Quit fabrica                                     |

### Keybindings

| Key                     | Action                                 |
|-------------------------|----------------------------------------|
| `Enter`                 | Send message                           |
| `Ctrl+C`                | Quit                                   |
| `↑` / `↓`               | Scroll conversation log                |
| `Page Up` / `Page Down` | Scroll by page                         |
| `Home`                  | Scroll to top                          |
| `End`                   | Jump to bottom (re-enable auto-scroll) |
| Mouse scroll            | Scroll conversation log                |

#### Model Picker

| Key       | Action          |
|-----------|-----------------|
| `↑` / `↓` | Navigate models |
| `Enter`   | Select model    |
| `Esc`     | Cancel          |

### Tools

The agent has access to the following tools and will use them automatically:

| Tool      | Description                                                          |
|-----------|----------------------------------------------------------------------|
| **bash**  | Execute shell commands (e.g. `ls`, `grep`, `find`, `git`, `cargo`)   |
| **read**  | Read file contents with optional line offset/limit                   |
| **write** | Create or overwrite files (creates parent directories automatically) |
| **edit**  | Make precise edits to existing files by line/column coordinates      |

## License

[MIT](LICENSE)
