# zjbar-opencode

[zjbar](https://github.com/imroc/zjbar) plugin for [OpenCode](https://opencode.ai) — live AI activity indicators in Zellij status bar.

## Install

1. Make sure you have [zjbar](https://github.com/imroc/zjbar) installed in your Zellij layout.

2. Add to your `opencode.json`:

```json
{
  "plugin": ["zjbar-opencode@latest"]
}
```

3. Start OpenCode inside a Zellij session with the zjbar layout — activity indicators will appear automatically.

## What it does

Translates OpenCode events into zjbar's unified event format via `zellij pipe`, enabling real-time activity indicators (thinking, tool use, waiting, etc.) on your Zellij tab bar.

The plugin only activates when running inside a Zellij session (`ZELLIJ_SESSION_NAME` is set).

## License

MIT
