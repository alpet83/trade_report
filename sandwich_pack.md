# Sandwich Pack CLI Utility Documentation

## Purpose

The `sandwich_pack` CLI utility is designed for **client-side analysis and packaging of complex software projects** to enable efficient processing by AI systems, such as large language models (LLMs). It transforms a project's source files into a structured, compact format called "sandwiches" and generates a comprehensive metadata index to facilitate AI-driven analysis, modification, or debugging. The utility is particularly useful for Rust projects but can be extended to support other languages.

Key objectives:
- **Compact Representation**: Package project files into manageable chunks (≤80 KB) to fit within LLM context limits (~27,500 tokens).
- **Structured Metadata**: Generate an index with file metadata, dependencies, and code entities (structs, traits, functions) to maintain project context.
- **Version Control**: Use MD5 hashes and modification timestamps to track file changes.
- **AI Compatibility**: Enable quick context restoration for AI interactions, even after context resets due to token limits.

## Usage

Run the utility from the project root:

```bash
python sandwich_pack.py
```

- **Input**: Scans the `./src` directory for source files (e.g., `.rs`, `.toml`) and the root for configuration files (e.g., `Cargo.toml`, `config.toml`).
- **Output**: Generates:
  - `sandwich_N.txt`: Text files containing source code wrapped in XML-like tags (`<rustc>`, `<config_toml>`).
  - `sandwiches_index.json`: A JSON index with metadata, dependencies, and entities for all files.

## Sandwich File Format

Each `sandwich_N.txt` file contains source code wrapped in tags based on file type:
- **Rust files** (`.rs`): `<rustc src="/path/to/file.rs" mod_time="YYYY-MM-DD HH:MM:SS EEST">...</rustc>`
- **TOML files** (`.toml`): `<config_toml src="/file.toml" mod_time="YYYY-MM-DD HH:MM:SS EEST">...</config_toml>`
- **Other files**: `<file src="/path/to/file" mod_time="YYYY-MM-DD HH:MM:SS EEST">...</file>` (not currently used).

Example:
```xml
<rustc src="/src/main.rs" mod_time="2025-06-20 09:01:06 EEST">
fn main() { ... }
</rustc>
<config_toml src="/config.toml" mod_time="2025-06-18 11:53:55 EEST">
[mysql]
url = "mysql://user:pass@localhost/trading"
</config_toml>
```

- **Size Limit**: Each sandwich is capped at 80 KB (~20,000 tokens) to ensure compatibility with LLM processing.
- **Encoding**: UTF-8, supporting multilingual comments (e.g., Cyrillic).

## Index File Format (`sandwiches_index.json`)

The `sandwiches_index.json` file is a JSON document that aggregates metadata for all project files across all sandwiches. It serves as the primary mechanism for maintaining project context and enables efficient navigation and analysis by AI systems.

### Structure

```json
{
  "project_version": "string",
  "context_date": "YYYY-MM-DD HH:MM:SS EEST",
  "sandwiches": [
    {
      "file": "sandwich_N.txt",
      "files": [
        {
          "src": "/path/to/file",
          "start_line": integer,
          "tokens": integer,
          "mod_time": "YYYY-MM-DD HH:MM:SS EEST",
          "md5": "hex_string",
          "dependencies": [
            {
              "type": "module|import|call",
              "details": "string",
              "src": "/path/to/dependency_file" (optional)
            }
          ],
          "entities": [
            {
              "type": "struct|trait|function",
              "name": "string"
            }
          ]
        }
      ]
    }
  ]
}
```

### Fields

- **project_version**: Project version from `Cargo.toml` (e.g., `"0.1.0"`).
- **context_date**: Timestamp of index generation (e.g., `"2025-06-21 17:16:17 EEST"`).
- **sandwiches**: Array of sandwich files.
  - **file**: Name of the sandwich file (e.g., `"sandwich_1.txt"`).
  - **files**: Array of file metadata.
    - **src**: Relative path to the source file (e.g., `/src/main.rs`).
    - **start_line**: Line number where the file's tag begins in the sandwich.
    - **tokens**: Estimated token count (content length ÷ 4).
    - **mod_time**: File modification timestamp (e.g., `"2025-06-20 09:01:06 EEST"`).
    - **md5**: MD5 hash of the tagged content (e.g., `<rustc>...</rustc>`).
    - **dependencies**: Array of dependencies.
      - **type**: Type of dependency (`module`, `import`, `call`).
      - **details**: Name of the module, imported item, or called function (e.g., `report`, `TradingAccount`).
      - **src**: Path to the dependency file, if resolved (e.g., `/src/api/report.rs`).
    - **entities**: Array of code entities (Rust-specific).
      - **type**: Entity type (`struct`, `trait`, `function`).
      - **name**: Entity name (e.g., `TradingAccount`, `generate_report`).

### Example

```json
{
  "project_version": "0.1.0",
  "context_date": "2025-06-21 17:16:17 EEST",
  "sandwiches": [
    {
      "file": "sandwich_1.txt",
      "files": [
        {
          "src": "/src/main.rs",
          "start_line": 1513,
          "tokens": 537,
          "mod_time": "2025-06-20 09:01:06 EEST",
          "md5": "3cee892f2ae27802829892a98fff817a",
          "dependencies": [],
          "entities": [
            {"type": "function", "name": "main"}
          ]
        },
        {
          "src": "/src/entities/account.rs",
          "start_line": 888,
          "tokens": 1387,
          "mod_time": "2025-06-21 13:14:01 EEST",
          "md5": "87f9dbf886f1e00ce86024929e30637c",
          "dependencies": [],
          "entities": [
            {"type": "struct", "name": "TradingAccount"},
            {"type": "struct", "name": "TradingAccountManager"},
            {"type": "function", "name": "resolve_account"}
          ]
        }
      ]
    }
  ]
}
```

## Context Restoration

To restore context in a new AI conversation:
1. **Load `sandwiches_index.json`**: Use it to identify files, their metadata, dependencies, and entities.
2. **Access `sandwich_N.txt`**: Navigate to specific files using `start_line` for targeted analysis.
3. **Track Changes**: Compare `md5` and `mod_time` to detect modifications.
4. **Analyze Dependencies**: Use `dependencies` to understand module relationships and imports.
5. **Inspect Entities**: Leverage `entities` to locate structs, traits, or functions.

If context is overloaded (~27,500 tokens), prioritize recent files (`mod_time`) or critical modules (e.g., `/src/lib.rs`). The index's compact size (~2,000 tokens) ensures it fits within most LLM contexts.

## Limitations

- **Dependency Resolution**: Imports and function calls may lack `src` paths due to heuristic limitations.
- **Entity Duplication**: Some functions may appear multiple times (e.g., trait implementations).
- **File Types**: Primarily supports `.rs` and `.toml`. Other formats require tag extensions (e.g., `<python>`).
- **Token Limits**: Large projects may require multiple sandwiches, but the index supports up to 12 portions (960 KB).

## Future Improvements

- Enhance dependency resolution with precise function call tracking.
- Filter duplicate entities in trait implementations.
- Support additional file types (e.g., `.py`, `.md`).
- Compress index with shorter field names (e.g., `s` for `src`).
- Version control for indexes (e.g., `index_YYYYMMDD_HHMMSS.json`).

## Contributing

To extend the utility:
- Add new tag types in `write_sandwich_files`.
- Improve `extract_entities` or `extract_dependencies` for other languages.
- Submit issues or PRs to the project repository.