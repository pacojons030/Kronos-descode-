# E2E Test Infrastructure: KRONOS DESCODE

## Test Philosophy
- **Requirement-Driven & Opaque-Box**: Tests are derived strictly from authoritative specifications (`ORIGINAL_REQUEST.md` R1-R6, Acceptance Criteria) and `PROJECT.md § Feature Inventory` (Features 1-21). Tests validate observable behaviors, contracts, schemas, file formats, and APIs without coupling to private internal implementation details.
- **Empirical Reference Oracles**: Expected outputs are derived from mathematical algorithms (Base64, Hex, XOR, ASCII/Chr arrays), specified theme tokens, session JSON schemas, and deterministic transformation results of `DeobfuscatorEngine`.
- **Methodology**: 4-Tier Test Pyramid + Category-Partitioning + Boundary Value Analysis + Pairwise Interaction Testing + Real-World Reverse Engineering Workloads + Adversarial Stress Testing.

---

## Feature Inventory & Test Coverage Matrix (21 Features)

| # | Feature | Requirement Source | Tier 1 (Feature Coverage) | Tier 2 (Boundary & Corner) | Tier 3 (Pairwise Cross-Feature) | Tier 4 (Workload Scenario) |
|---|---------|-------------------|:-------------------------:|:--------------------------:|:-------------------------------:|:--------------------------:|
| 1 | Modular Rust Architecture | ORIGINAL_REQUEST §R6 | T1.F1.1 - T1.F1.5 | T2.F1.1 - T2.F1.5 | T3.1, T3.7 | T4.5 |
| 2 | Core Engine Preservation | ORIGINAL_REQUEST §R6, AC | T1.F2.1 - T1.F2.5 | T2.F2.1 - T2.F2.5 | T3.1, T3.2, T3.6 | T4.1, T4.2, T4.3, T4.4 |
| 3 | Session State Persistence | ORIGINAL_REQUEST §R5, AC | T1.F3.1 - T1.F3.5 | T2.F3.1 - T2.F3.5 | T3.3 | T4.5 |
| 4 | Native Gemini REST Client | ORIGINAL_REQUEST §R4, AC | T1.F4.1 - T1.F4.5 | T2.F4.1 - T2.F4.5 | T3.1, T3.8, T3.11, T3.12 | T4.1, T4.7, T4.9 |
| 5 | Model & Mode Selection | ORIGINAL_REQUEST §R4, AC | T1.F5.1 - T1.F5.5 | T2.F5.1 - T2.F5.5 | T3.12 | T4.7 |
| 6 | Contextual AI Actions | ORIGINAL_REQUEST §R4, AC | T1.F6.1 - T1.F6.5 | T2.F6.1 - T2.F6.5 | T3.1, T3.8 | T4.1, T4.9 |
| 7 | Extensible AI Provider Trait | ORIGINAL_REQUEST §R4 | T1.F7.1 - T1.F7.5 | T2.F7.1 - T2.F7.5 | T3.11 | T4.7 |
| 8 | Sovereign Obsidian & Crimson Theme | ORIGINAL_REQUEST §R2, AC | T1.F8.1 - T1.F8.5 | T2.F8.1 - T2.F8.5 | T3.10 | T4.5 |
| 9 | Compact Top Bar | ORIGINAL_REQUEST §R1 | T1.F9.1 - T1.F9.5 | T2.F9.1 - T2.F9.5 | T3.7 | T4.5 |
| 10 | Activity Bar Navigation | ORIGINAL_REQUEST §R1, R2 | T1.F10.1 - T1.F10.5 | T2.F10.1 - T2.F10.5 | T3.4 | T4.5 |
| 11 | Collapsible Context Panel | ORIGINAL_REQUEST §R1, R5 | T1.F11.1 - T1.F11.5 | T2.F11.1 - T2.F11.5 | T3.4 | T4.5 |
| 12 | Integrated KRONOS AI Panel | ORIGINAL_REQUEST §R1, R4 | T1.F12.1 - T1.F12.5 | T2.F12.1 - T2.F12.5 | T3.8, T3.11 | T4.1, T4.9 |
| 13 | Multi-Tab Central Workspace | ORIGINAL_REQUEST §R1, R3, R5 | T1.F13.1 - T1.F13.5 | T2.F13.1 - T2.F13.5 | T3.3, T3.9, T3.14 | T4.5, T4.10 |
| 14 | AST Visual Explorer | ORIGINAL_REQUEST §R3, AC | T1.F14.1 - T1.F14.5 | T2.F14.1 - T2.F14.5 | T3.5 | T4.3 |
| 15 | Visual Code Diff Viewer | ORIGINAL_REQUEST §R3, AC | T1.F15.1 - T1.F15.5 | T2.F15.1 - T2.F15.5 | T3.2, T3.10, T3.15 | T4.1, T4.2, T4.8 |
| 16 | Pipeline Configuration & Execution | ORIGINAL_REQUEST §R3 | T1.F16.1 - T1.F16.5 | T2.F16.1 - T2.F16.5 | T3.1, T3.2, T3.7, T3.13 | T4.1, T4.2, T4.4 |
| 17 | Analysis History Log | ORIGINAL_REQUEST §R3 | T1.F17.1 - T1.F17.5 | T2.F17.1 - T2.F17.5 | T3.6, T3.15 | T4.1, T4.5 |
| 18 | Asynchronous Threading (Zero Freeze) | ORIGINAL_REQUEST §R5, AC | T1.F18.1 - T1.F18.5 | T2.F18.1 - T2.F18.5 | T3.11 | T4.1, T4.7 |
| 19 | Command Palette & Shortcuts | ORIGINAL_REQUEST §R5 | T1.F19.1 - T1.F19.5 | T2.F19.1 - T2.F19.5 | T3.7 | T4.5 |
| 20 | E2E Testing Suite (Tiers 1-4) | Dual Track, PROJECT.md | T1.F20.1 - T1.F20.5 | T2.F20.1 - T2.F20.5 | T3.1 - T3.15 | T4.1 - T4.10 |
| 21 | 100% E2E Pass & Adversarial Hardening | Dual Track, PROJECT.md | T1.F21.1 - T1.F21.5 | T2.F21.1 - T2.F21.5 | T3.1 - T3.15 | T4.6 |

---

## 4-Tier Test Architecture

### Tier 1: Feature Coverage (>=5 test cases per feature in isolation)
- Total tests: 105 tests (5 test cases x 21 features).
- Focus: Isolated verification of primary functional requirements, data structures, return values, and happy paths.

### Tier 2: Boundary & Corner Cases (>=5 test cases per feature)
- Total tests: 105 tests (5 test cases x 21 features).
- Focus: Empty inputs, boundary values, extreme dimensions, syntax bombs, malformed tokens, network timeouts, invalid keys, non-ASCII Unicode, and resource limits.

### Tier 3: Pairwise Cross-Feature Interactions
- Total tests: 15 integration tests.
- Focus: Contract handoffs between modules (e.g. Deobfuscator + AI prompt packaging, Pipeline + Diff viewer, Session persistence + Tab manager, Activity Bar + Context Drawer synchronization).

### Tier 4: Real-World Reverse Engineering Workloads
- Total tests: 10 end-to-end scenarios.
- Focus: Realistic malware droppers, multi-layer base64/xor packers, PyArmor eval/exec constructs, control-flow flattening, airgap offline failovers, and multi-file project workspace lifecycles.

---

## Test Verification Architecture & Commands

```bash
# 1. Execute entire E2E test suite (Tiers 1-4)
cargo test --test e2e_suite --manifest-path c:\Users\Igor2\Desktop\kronos_ast\Cargo.toml

# 2. Run unit tests and core verification
cargo test --manifest-path c:\Users\Igor2\Desktop\kronos_ast\Cargo.toml

# 3. Compile check for release build
cargo build --release --manifest-path c:\Users\Igor2\Desktop\kronos_ast\Cargo.toml
```

## Directory Layout of Test Suite
```
c:\Users\Igor2\Desktop\kronos_ast\tests\
├── common\
│   └── mod.rs                         # Common fixtures, models, AST helpers, Slint validators
├── e2e\
│   ├── mod.rs                         # Aggregator module
│   ├── tier1_feature_coverage.rs      # 105 Tier 1 test cases
│   ├── tier2_boundary_corner.rs       # 105 Tier 2 test cases
│   ├── tier3_pairwise_cross_feature.rs # 15 Tier 3 pairwise test cases
│   └── tier4_real_world_workloads.rs   # 10 Tier 4 workload scenarios
└── e2e_suite.rs                       # Master integration test runner
```
