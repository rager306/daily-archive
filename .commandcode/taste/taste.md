# Taste (Continuously Learned by [CommandCode][cmd])

[cmd]: https://commandcode.ai/

# mcp
- Use project-level MCP configuration via .mcp.json rather than global MCP config files. Confidence: 0.65

# cli
- Use `uv run python` instead of bare `python3` for running Python commands in this project. Confidence: 0.65

# workflow
- When user makes a concrete actionable request alongside an open-ended one, prioritize the concrete task first before launching deep exploratory analysis. Confidence: 0.80
- For complex multi-tool research or analysis tasks, delegate to subagents and pass MCP permissions so they can use all available MCP tools independently. Confidence: 0.65

# communication
- Do not guess or fabricate answers when uncertain; research first using available tools, and if evidence isn't found, honestly say "I don't know". Confidence: 0.85
- Refer to self as CommandCode, not as Claude Code. Confidence: 0.80


