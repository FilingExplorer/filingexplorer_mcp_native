# FilingExplorer MCP

A native [Model Context Protocol](https://modelcontextprotocol.io/) server that gives [Claude Desktop](https://claude.ai/download) access to SEC filings, company financials, institutional holdings, and more through the [FilingExplorer API](https://www.filingexplorer.com).

## Features

- **Institutional Holdings** — Form 13F submissions from hedge funds and asset managers
- **Insider Trading** — Form 4 transactions from executives and directors
- **Investment Advisers** — Form ADV data including AUM, ownership, and disclosures
- **SEC Filings** — 13F, 4, 10-K, 10-Q, 8-K, and other SEC documents
- **Company Financials** — Balance sheets, income statements, cash flow from standardized filings
- **ETF Holdings** — Portfolio composition from N-PORT filings
- **Lobbying Data** — Federal lobbying disclosures and spending patterns
- **Watchlists** — Track companies and institutional investors

## Installation

### macOS

1. Download the latest `.dmg` from [Releases](https://github.com/FilingExplorer/filing-explorer-mcp/releases)
2. Open the DMG and drag **FilingExplorer Settings** to Applications
3. Open the app and enter your [FilingExplorer API token](https://filingexplorer.com/api-keys)
4. To use the tools that fetch documents directly from the SEC, enter your email you@your-company-domain.com -- (Email address with private domain required by SEC for EDGAR access).
5. Click **Configure Claude Desktop**
6. Restart Claude Desktop

### Manual Setup

For headless environments or if you prefer manual configuration:

**1. Create the config file:**

```bash
mkdir -p ~/Library/Application\ Support/com.filingexplorer.mcp

cat > ~/Library/Application\ Support/com.filingexplorer.mcp/config.json << 'EOF'
{
  "api_token": "YOUR_FILINGEXPLORER_API_TOKEN",
  "sec_user_agent_name": "Your Name or Company",
  "sec_user_agent_email": "your@email.com"
}
EOF
```

**2. Add to Claude Desktop config** (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "filing-explorer": {
      "command": "/Applications/FilingExplorer Settings.app/Contents/MacOS/mcp-server",
      "args": []
    }
  }
}
```

**3. Restart Claude Desktop**

## Usage

Once configured, ask Claude about SEC filings:

- "Show me the top ten and bottom ten holdings by weight in the ETF VXUS and format these in a CSV with their percentage"
- "Can you look for any major insider trades at Apple this month?"
- "Show me Berkshire Hathaway's latest 13F holdings"
- "Get Microsoft's 10-K from 2024 so we can discuss it"

The server uses progressive discovery—Claude will first search available tools, then execute the appropriate one.

## Building from Source

Requires Rust 1.75+ and Node.js 18+.

```bash
# Build universal macOS binary (Apple Silicon + Intel)
./build-macos-universal.sh
```

The app will be at `target/universal-apple-darwin/release/bundle/macos/FilingExplorer Settings.app`

## License

MIT
