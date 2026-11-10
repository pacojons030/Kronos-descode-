# Project: KRONOS DESCODE Desktop Environment Reconstruction

## Architecture
KRONOS DESCODE is a sovereign, minimalist desktop environment for reverse engineering, deobfuscation, AST analysis, and code inspection built on Rust and Slint with an integrated native Google Gemini contextual AI assistant.

### Module Boundaries
1. `src/core/`: Pure deobfuscation engine, 7 static decoders, identifier normalization, multi-pass pipeline, security scanner, zero-exec guardrails. (100% preserved from original codebase).
2. `src/ast/`: Abstract Syntax Tree representation, visitor nodes, token inspection, and visual hierarchy generation.
3. `src/ai/`: Extensible `AiProvider` trait, native Google Gemini REST API client, model selector (`gemini-3.1-flash-lite`, `gemini-3.8-flash`), contextual actions (explain, analyze, AST interpret, obfuscation detection, reconstruction suggestions).
4. `src/analysis/`: Reverse engineering analytics, operation history, security vulnerability reports, diff metrics.
5. `src/fs/`: File system abstractions, workspace directory recursion, file reading/writing, tab file tracking.
6. `src/project/`: Project workspace lifecycle, recent projects, project metadata.
7. `src/config/`: User settings, API keys, window geometry, session state serialization to `%APPDATA%/kronos_descode/session.json`.
8. `src/ui/`: Rust Slint controllers, event bridge, asynchronous worker dispatch via channels (`slint::invoke_from_event_loop`), clipboard integration (`arboard`).
9. `ui/`: Modular Slint UI architecture (`theme.slint`, `top_bar.slint`, `activity_bar.slint`, `context_panel.slint`, `workspace.slint`, `ai_panel.slint`, `command_palette.slint`, `components/`).

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Modular Rust Architecture | Clean separation into UI, Core, AI, AST, Analysis, File Mgmt, Project Mgmt, Configuration | M1 | R6 |
| 2 | Core Engine Preservation | Strict retention of 7 static decoders, normalizer, pipeline, security scanner with 12/12 unit tests passing | M1 | R6, AC |
| 3 | Session State Persistence | Save/restore workspace layout, panel sizes, open tabs, active tools to session.json | M1, M4 | R5, AC |
| 4 | Native Gemini REST Client | Direct HTTPS client to Gemini API with API key config, replacing curl.exe | M2 | R4, AC |
| 5 | Model & Mode Selection | Switch between gemini-3.1-flash-lite, gemini-3.8-flash, and task modes | M2 | R4, AC |
| 6 | Contextual AI Actions | Explain selected code, analyze current file, interpret AST, identify obfuscation, suggest reconstruction | M2 | R4, AC |
| 7 | Extensible AI Provider Trait | Architecture for pluggable AI engines without simulated mocks | M2 | R4 |
| 8 | Sovereign Obsidian & Crimson Theme | Palette: #0B0B0D, #101013, #151518, #0D0D10, #26262B, #E8E8EB, #85858F, #D9283D, #F13B50; 1px borders; zero neon/gradients/glassmorphism | M3 | R2, AC |
| 9 | Compact Top Bar | Horizontal bar with logo, menus, project info, global search, window controls | M3 | R1 |
| 10 | Activity Bar Navigation | Narrow vertical bar with minimalist icons and subtle crimson active indicator | M3 | R1 |
| 11 | Collapsible Context Panel | Resizable side drawer for active tool (Explorer, Reverse Eng, AST, Pipeline, History) | M3, M4 | R1, R5 |
| 12 | Integrated KRONOS AI Panel | Collapsible, resizable right panel with chat history, model selector, context badge, code blocks | M3, M4 | R1, R4 |
| 13 | Multi-Tab Central Workspace | Flexible central area with file tabs, syntax-highlighted editor, decoupled result panels | M3, M4 | R1, R3 |
| 14 | AST Visual Explorer | Interactive visual tree representation of parsed code AST | M4 | R3, AC |
| 15 | Visual Code Diff Viewer | Side-by-side / inline comparison of original vs deobfuscated / reconstructed code | M4 | R3, AC |
| 16 | Pipeline Configuration & Execution | Multi-pass transformation configuration and execution | M4 | R3 |
| 17 | Analysis History Log | History of operations, security audits, and transformation reports | M4 | R3 |
| 18 | Asynchronous Threading (Zero Freeze) | Offload heavy AST, deobfuscation, and AI network calls to background threads via channels | M4 | R5, AC |
| 19 | Command Palette & Shortcuts | Keyboard shortcuts and quick command overlay | M4 | R5 |
| 20 | E2E Testing Suite (Tiers 1-4) | Comprehensive opaque-box test suite verifying all acceptance criteria | E2E Track | Dual Track |
| 21 | 100% E2E Pass & Adversarial Hardening | Verification of 100% pass on all test tiers + Tier 5 adversarial tests | M5 | Dual Track |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Architectural Modularization & Persistence | Modularize Rust codebase into 8 clean modules, preserve core 12/12 unit tests, implement session persistence | none | PLANNED |
| M2 | Native Gemini AI Engine | Implement AiProvider trait, native Gemini REST client, model selector, contextual actions | M1 | PLANNED |
| M3 | Slint Sovereign UI Overhaul | Implement Obsidian & Crimson theme, modular Slint components, top bar, activity bar, resizable panels | M1 | PLANNED |
| M4 | Tool Integration & Async Threading | Connect AST Explorer, Code Diff, Pipeline, History, Command Palette, and async worker channels | M2, M3 | PLANNED |
| M5 | Final Milestone: 100% E2E Pass & Hardening | Pass 100% of E2E test suite (Tiers 1-4) and complete Tier 5 adversarial hardening | M4, E2E Track | PLANNED |
| E2E | E2E Testing Track | Independent opaque-box test suite derived from requirements (Tiers 1-4) publishing TEST_READY.md | none | IN_PROGRESS |

## Interface Contracts
### `src/core/` ↔ `src/ui/` / `src/ast/`
- `DeobfuscatorEngine::process(input: &str, config: &DeobfuscationConfig) -> DeobfuscationResult`
- `SecurityScanner::scan(input: &str) -> SecurityAudit`
- Pure, deterministic, zero UI dependencies.

### `src/ai/` ↔ `src/ui/`
- `AiProvider::query(&self, request: &AiQueryRequest) -> Result<AiQueryResponse, AiError>`
- `AiAction`: `ExplainCode`, `AnalyzeFile`, `InterpretAst`, `DetectObfuscation`, `SuggestReconstruction`
- Asynchronous execution offloaded via background thread/tokio, returning results to UI via `slint::invoke_from_event_loop`.

### `src/config/` ↔ All Modules
- `SessionState`: layout dimensions, splitter ratios, open tabs, active tool, recent projects.
- `AppConfig`: Gemini API key, default model, theme preferences.

## Code Layout
```
c:\Users\Igor2\Desktop\kronos_ast\
├── Cargo.toml
├── build.rs
├── ui/
│   ├── app.slint
│   ├── theme.slint
│   ├── top_bar.slint
│   ├── activity_bar.slint
│   ├── context_panel.slint
│   ├── workspace.slint
│   ├── ai_panel.slint
│   ├── command_palette.slint
│   └── components/
│       ├── button.slint
│       ├── tab.slint
│       ├── splitter.slint
│       └── diff_view.slint
└── src/
    ├── main.rs
    ├── core/
    │   ├── mod.rs
    │   ├── decoders.rs
    │   ├── normalizer.rs
    │   ├── pipeline.rs
    │   ├── security.rs
    │   └── samples.rs
    ├── ast/
    │   ├── mod.rs
    │   └── tree.rs
    ├── ai/
    │   ├── mod.rs
    │   ├── provider.rs
    │   └── gemini.rs
    ├── analysis/
    │   ├── mod.rs
    │   └── history.rs
    ├── fs/
    │   ├── mod.rs
    │   └── workspace.rs
    ├── project/
    │   ├── mod.rs
    │   └── manager.rs
    ├── config/
    │   ├── mod.rs
    │   └── session.rs
    └── ui/
        ├── mod.rs
        ├── controller.rs
        └── async_worker.rs
```
