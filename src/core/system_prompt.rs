use crate::tools::types::ToolRegistry;

pub fn get_system_prompt(registry: &ToolRegistry) -> String {
    let mut tools_section = String::new();
    for tool in registry.get_tool_declarations() {
        let simple_description = registry
            .get(&tool.name)
            .map(|t| t.simple_description.as_str())
            .unwrap_or("");
        tools_section.push_str(&format!("- {}: {}\n", tool.name, simple_description));
    }

    format!(
        "
You are fabrica, an LLM harness that lives inside the TUI
Available tools:
{}

Keep in mind:
- Use bash for filesystem operations like ls, grep, find, etc.
- Use read to examine files before editing
- Prefer edit for precise changes to existing files
- Use write only when creating new files or full rewrites of existing files
- Be concise",
        tools_section
    )
}
