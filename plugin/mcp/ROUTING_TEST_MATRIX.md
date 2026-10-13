# `/gtfs` independent routing test matrix

This matrix tests the routing contract without relying on an earlier chat
conversation. It distinguishes model routing from MCP execution:

- A routing test passes only when the connected ChatGPT session emits the
  expected MCP tool call with the expected arguments.
- A successful `tools/list` or direct MCP call proves server availability and
  schema correctness, but does not prove that ChatGPT selected the tool.
- The MCP server cannot register a native slash-command menu item. `/gtfs` is
  tested as user-message intent text.

## Preconditions

1. Start a new ChatGPT conversation with no prior GTFS context.
2. Connect the current GTFS Validator MCP server.
3. Enable tool-call trace or inspect the MCP request log.
4. Use a small valid GTFS Schedule ZIP for upload cases and a public,
   directly downloadable GTFS ZIP URL for URL cases.

## Positive routing cases

| ID | User message | Input | Expected MCP call |
|---|---|---|---|
| R-01 | `/gtfs` | Attached ZIP | `analyze_gtfs_file(file=...)` |
| R-02 | `/gtfs https://example.test/feed.zip` | No attachment | `analyze_gtfs_url(url=...)` |
| R-03 | `/gtfs TRP_002` | No attachment | `get_gtfs_rule(rule_id="TRP_002")` |
| R-04 | `/gtfs` | No attachment or URL | No tool call; ask for a ZIP or public URL |
| R-05 | `Bu GTFS dosyasını analiz et` | Attached ZIP | `analyze_gtfs_file(file=...)` |
| R-06 | `Validate this GTFS ZIP` | Attached ZIP | `analyze_gtfs_file(file=...)` |
| R-07 | `GTFS URL'sini kontrol et: https://example.test/feed.zip` | No attachment | `analyze_gtfs_url(url=...)` |
| R-08 | `Explain rule TRP_002` | No attachment | `get_gtfs_rule(rule_id="TRP_002")` |

Repeat R-05 through R-08 with the same intent expressed in Turkish, English,
Japanese, and French. The final explanation language may vary, but the selected
MCP tool and input mode must remain correct.

## Negative and boundary cases

| ID | User message | Expected behavior |
|---|---|---|
| N-01 | `List the files inside this ZIP` | Do not invoke GTFS Validator solely for archive listing |
| N-02 | `/gtfs` + private or localhost URL | `analyze_gtfs_url` may be selected, then server returns URL-security error |
| N-03 | `/gtfs NOPE_999` | `get_gtfs_rule` returns `UNKNOWN_RULE` |
| N-04 | `Analyze this file` with no attachment | Ask for the missing file; do not fabricate a call argument |
| N-05 | `/gtfs` + attachment and URL together | Confirm which source to use; do not silently analyze the wrong input |

## MCP-only preflight

Before interpreting model results, verify independently:

```text
initialize → server instructions contain /gtfs and tr/en/ja/fr
tools/list → exactly analyze_gtfs_file, analyze_gtfs_url, get_gtfs_rule
tools/list → analyze_gtfs_file advertises openai/fileParams=["file"]
tools/call with missing required fields → isError=true
```

The preflight validates the server contract. R-01 through R-08 are the
separate, context-free tests of whether the ChatGPT client actually converts
the messages into those MCP calls.
