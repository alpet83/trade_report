# Code Lossless Assistance (CLA) Rules

**Date**: 2025-06-24  
**Purpose**: To define the guidelines for providing code-related assistance in a way that ensures accuracy, clarity, and minimal data loss when working with user-provided code, project files, and development tasks.

## Overview
The Code Lossless Assistance (CLA) rules are designed to ensure that responses involving code (e.g., debugging, feature development, unit testing) preserve the integrity of the user’s project, provide clear and accurate solutions, and minimize context overload. These rules are particularly relevant for projects like `trade_report`, where iterative code changes, file synchronization, and detailed debugging are critical.

## Rules

1. **Code Synchronization**:
   - Always synchronize with the latest user-provided files (e.g., `sandwich_2.txt`, `sandwiches_index.json`) and verify MD5 hashes against the repository (e.g., https://github.com/alpet83/trade_report).
   - Reference specific files and their artifact IDs (e.g., `trades_aggregator.rs`, artifact_id: 9cc7b6ea-5269-4c75-9753-636a5680e4ed) to ensure accuracy.
   - If files are missing or outdated, request clarification from the user before proceeding.

2. **Minimal Context**:
   - Responses should focus on the current request, referencing only relevant prior interactions to avoid context overload.
   - Use `context_backup.md` or `dialogue_backup.md` (e.g., artifact_id: 57e0a41b-e83d-4f3f-af76-81a7c93bf729) to summarize past interactions when needed.
   - Avoid including unnecessary code or history unless explicitly requested.

3. **Code Integrity**:
   - Provide code changes as complete, self-contained artifacts (e.g., full file updates with artifact IDs).
   - Preserve existing functionality unless explicitly instructed to modify or remove it.
   - Include comments in code to explain changes (e.g., `// Modified: 2025-06-24 17:00 EEST`).
   - Avoid premature changes (e.g., adding features like `generate_tax_report` before user confirmation).

4. **Error Handling and Debugging**:
   - Address errors (e.g., `no method named 'clone' for TradesCache`, `Invalid timestamp`) by analyzing logs and user input.
   - Provide detailed steps to reproduce and fix errors, including test commands:
     ```powershell
     $env:RUST_LOG = "trade_report=debug"
     cargo test -- --nocapture | Out-File -Encoding utf8 output.txt
     ```
   - Suggest logging and database checks (e.g., `SELECT * FROM bitmex__trades WHERE pair_id = 1 LIMIT 10;`).

5. **Testing**:
   - Ensure all code changes are accompanied by unit tests (e.g., `test_trades_aggregator_coarse` in `tests/trades_aggregator.rs`).
   - Validate tests against user-provided data (e.g., `sample_trades.csv`, `expected_results.json`).
   - Include expected test logs to confirm success:
     ```
     INFO trade_report::tests::trades_aggregator: Successfully tested TradesAggregator in Coarse mode
     ```

6. **Backup and Context Management**:
   - Generate hourly dialogue backups (`dialogue_backup.md`) with context tags (`отладка`, `развитие функционала`, `развитие юнит-теста`, `UX`, `бэкап`).
   - Create detailed context backups (`context_backup.md`) when requested, covering dialogue, code changes, test statuses, and recommendations.
   - Recommend starting new chats with reference to backup artifacts to reduce context overload.

7. **User-Centric Approach**:
   - Respect user preferences (e.g., avoiding premature features, focusing on debugging).
   - Provide actionable recommendations (e.g., P/L calculation, CSV export) with example code snippets.
   - Address UX concerns (e.g., tree-like UI proposals) when requested.

8. **Transparency**:
   - Notify users of known server issues (e.g., X platform outage on 2025-05-25) if relevant to response delays.[](https://www.reuters.com/business/musks-x-down-tens-thousands-us-users-downdetector-shows-2025-05-24/)
   - Cite sources for external data (e.g., `` for xAI status) using the specified format.[](https://status.x.ai/)

9. **Version Control**:
   - Reference the project version (e.g., `trade_report v0.3.0`) and repository state (e.g., last synced 2025-06-24 15:26 EEST).
   - Offer to compare MD5 hashes with the repository if requested.

10. **Future Improvements**:
    - Suggest enhancements to the development process (e.g., integrating `TradesAggregator` into `Exchange::new`, adding P/L calculations).
    - Propose UX improvements (e.g., Reddit-style UI for development chats) when relevant.

## Usage
- **New Chat**: Start a new chat with:
  ```markdown
  Use `context_backup.md` (artifact_id: 59ddbf2e-1719-4eb0-8a9b-93facca2a716), `cla_rules.md` (artifact_id: 7f3c9a7e-6b2d-4e1f-9b7c-2e3a4b5c6d7e), and specified files from `sandwich_2.txt`. Generate hourly backup.
  ```
- **Error Reporting**: If server issues persist, report via https://x.ai/support with details (e.g., "Grok response delays on 2025-06-24").