# Semfora Engine

Semantic code analyzer that produces compressed TOON (Text Object-Oriented Notation) output for AI-assisted code review. Extracts symbols, dependencies, control flow, state changes, and risk assessments from source files.

## Installation

```bash
cargo build --release
```

## Binaries

The project builds three binaries:

| Binary | Purpose |
|--------|---------|
| `semfora-engine` | CLI for semantic analysis, indexing, and querying |
| `semfora-engine-server` | MCP server for AI agent integration |
| `semfora-daemon` | WebSocket daemon for real-time updates |

## Usage

```bash
# Analyze a single file
semfora-engine path/to/file.rs

# Analyze a directory and create sharded index
semfora-engine --dir . --shard

# Query the index
semfora-engine --search-symbols "authenticate"

# Start MCP server (for AI agents)
semfora-engine-server

# Start WebSocket daemon (for real-time updates)
semfora-daemon
```

See [CLI Reference](docs/cli.md) for full documentation.

## Supported Languages

### Programming Languages

| Language               | Extensions                                   | Family     | Implementation Details                                                                   |
| ---------------------- | -------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------- |
| **TypeScript**         | `.ts`, `.mts`, `.cts`                        | JavaScript | Full AST extraction via `tree-sitter-typescript`; exports, interfaces, enums, decorators |
| **TSX**                | `.tsx`                                       | JavaScript | TypeScript + JSX/React component detection, hooks, styled-components                     |
| **JavaScript**         | `.js`, `.mjs`, `.cjs`                        | JavaScript | Functions, classes, imports; framework detection for React, Express, Angular             |
| **JSX**                | `.jsx`                                       | JavaScript | JavaScript + JSX component detection                                                     |
| **Rust**               | `.rs`                                        | Rust       | Functions, structs, traits, enums; `pub` visibility detection via `tree-sitter-rust`     |
| **Python**             | `.py`, `.pyi`                                | Python     | Functions, classes, decorators; underscore-prefix privacy convention                     |
| **Go**                 | `.go`                                        | Go         | Functions, methods, structs; uppercase-export convention via `tree-sitter-go`            |
| **Java**               | `.java`                                      | Java       | Classes, interfaces, enums, methods; visibility modifiers                                |
| **Kotlin**             | `.kt`, `.kts`                                | Kotlin     | Classes, functions, objects; visibility modifiers via `tree-sitter-kotlin-ng`            |
| **C**                  | `.c`, `.h`                                   | C Family   | Functions, structs, enums; macro and `extern` detection via `tree-sitter-c`              |
| **C++**                | `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`, `.hh` | C Family   | Classes, templates, RAII patterns via `tree-sitter-cpp`                                  |
| **Assembly (Generic)** | `.s`, `.asm`, `.S`                           | Low-level  | Instruction blocks, labels, directives via `tree-sitter-asm`                             |
| **Shell / Bash**       | `.sh`, `.bash`, `.zsh`, `.fish`              | Shell      | Functions, variable assignments, command invocations via `tree-sitter-bash`              |
| **Gradle (Groovy)**    | `.gradle`                                    | JVM Build  | Groovy-based build files via `tree-sitter-groovy`                                        |

---

### Build Systems & Tooling Languages

These are critical for large C, C++, embedded, and retro-console codebases.

| Language / Format            | Extensions                 | Purpose          | Implementation Details                                   |
| ---------------------------- | -------------------------- | ---------------- | -------------------------------------------------------- |
| **Makefile**                 | `Makefile`, `.mk`          | Build system     | Target graph, recipes, variables via `tree-sitter-make`  |
| **CMake**                    | `CMakeLists.txt`, `.cmake` | Build system     | Target definitions, dependencies via `tree-sitter-cmake` |
| **GNU Linker Scripts**       | `.ld`                      | Toolchain        | Structural parsing only (no semantic pass yet)           |
| **GCC Attributes & Pragmas** | inline in C/C++            | Compiler control | Parsed as part of C/C++ AST                              |

---

### Framework Detection (JavaScript Family)

| Framework   | Detection Method            | Extracted Information                         |
| ----------- | --------------------------- | --------------------------------------------- |
| **React**   | Import from `react`         | Components, hooks, forwardRef, memo           |
| **Next.js** | `/app/`, `/pages/` patterns | API routes, layouts, server/client components |
| **Express** | Import from `express`       | Route handlers, middleware                    |
| **Angular** | Decorators (`@Component`)   | Components, services, modules                 |
| **Vue**     | `.vue` files                | SFC script extraction, Composition API        |

---

### Markup & Styling

| Language        | Extensions         | Implementation Details                           |
| --------------- | ------------------ | ------------------------------------------------ |
| **HTML**        | `.html`, `.htm`    | DOM structure via `tree-sitter-html`             |
| **CSS**         | `.css`             | Stylesheet structure via `tree-sitter-css`       |
| **SCSS / SASS** | `.scss`, `.sass`   | Nested rules via `tree-sitter-scss`              |
| **Markdown**    | `.md`, `.markdown` | Section and block structure via `tree-sitter-md` |

---

### Configuration & Data

| Language            | Extensions                       | Implementation Details                    |
| ------------------- | -------------------------------- | ----------------------------------------- |
| **JSON**            | `.json`                          | Structural parsing via `tree-sitter-json` |
| **YAML**            | `.yaml`, `.yml`                  | Structural parsing via `tree-sitter-yaml` |
| **TOML**            | `.toml`                          | Config parsing via `tree-sitter-toml-ng`  |
| **XML**             | `.xml`, `.svg`, `.plist`, `.pom` | Tree structure via `tree-sitter-xml`      |
| **HCL / Terraform** | `.tf`, `.hcl`, `.tfvars`         | IaC parsing via `tree-sitter-hcl`         |

---

### Single-File Components

| Format      | Extension | Implementation Details                        |
| ----------- | --------- | --------------------------------------------- |
| **Vue SFC** | `.vue`    | Script extraction with language-aware parsing |

---

## Duplicate Detection & Boilerplate Patterns

Semfora Engine includes semantic duplicate detection that identifies structurally similar code while filtering expected boilerplate.

### Current Boilerplate Coverage

| Language                | Patterns | Status          |
| ----------------------- | -------- | --------------- |
| JavaScript / TypeScript | 19       | Full support    |
| Rust                    | 13       | Full support    |
| C#                      | 18       | Full support    |
| Python                  | 0        | Planned         |
| Go                      | 0        | Planned         |
| Java                    | 0        | Planned         |
| C / C++                 | 0        | Planned         |
| Assembly                | N/A      | Structural only |

---

### Planned Boilerplate Patterns

| Language     | Planned Patterns                                              | Priority |
| ------------ | ------------------------------------------------------------- | -------- |
| **Python**   | pytest fixtures, dataclasses, FastAPI routes, Pydantic models | High     |
| **Go**       | HTTP handlers, middleware, error wrapping                     | High     |
| **Java**     | Spring controllers, Lombok, DTOs, JPA entities                | High     |
| **C / C++**  | RAII wrappers, copy/move boilerplate, driver init blocks      | High     |
| **Kotlin**   | Data classes, coroutines, Ktor routing                        | High     |
| **Makefile** | Repeated build targets, recursive includes                    | Medium   |

---

## Language Roadmap

Prioritized by enterprise relevance, embedded systems reach, and large-repo payoff.

### Completed

* C#
* HCL / Terraform
* JavaScript / TypeScript
* Rust
* Core C / C++

---

### Priority 1: Deep C / C++ Expansion

Critical for:

* Embedded systems
* Emulators
* Operating systems
* Retro-console SDKs (KallistiOS, SDL ports)

| Focus                    | Details                                    |
| ------------------------ | ------------------------------------------ |
| **Assembly Integration** | SH-4, ARM, x86 inline asm correlation      |
| **Driver Patterns**      | IRQ handlers, register maps, init/shutdown |
| **Build Graphs**         | Makefile + CMake cross-analysis            |

---

### Priority 2: Kotlin

| Item    | Details                                      |
| ------- | -------------------------------------------- |
| Parser  | `tree-sitter-kotlin-ng`                      |
| Targets | Coroutines, sealed classes, Android + server |

---

### Priority 3: Swift

| Item    | Details                         |
| ------- | ------------------------------- |
| Parser  | `tree-sitter-swift`             |
| Targets | Protocols, SwiftUI, async/await |

---

### Priority 4: PHP

| Item    | Details            |
| ------- | ------------------ |
| Parser  | `tree-sitter-php`  |
| Targets | Laravel, WordPress |

---

### Priority 5: Infra & Tooling (Structural)

| Language       | Extensions   | Mode       |
| -------------- | ------------ | ---------- |
| Dockerfile     | `Dockerfile` | Structural |
| PowerShell     | `.ps1`       | Structural |
| Linker scripts | `.ld`        | Structural |

---

## Known Unsupported Formats

| Format           | Extensions    | Reason                       |
| ---------------- | ------------- | ---------------------------- |
| Jest Snapshots   | `.shot`       | Test artifacts               |
| MDX              | `.mdx`        | Hybrid JSX + Markdown        |
| AsciiDoc         | `.adoc`       | Docs-only                    |
| Protocol Buffers | `.proto`      | Tree-sitter version mismatch |
| Scala            | `.scala`      | Low demand vs complexity     |
| Elixir           | `.ex`, `.exs` | Low enterprise priority      |

---

## Architecture

```
src/
├── main.rs              # CLI entry point (semfora-engine binary)
├── cli.rs               # CLI argument definitions
├── lib.rs               # Library exports
├── lang.rs              # Language detection from file extensions
├── extract.rs           # Main extraction orchestration
├── schema.rs            # SemanticSummary output schema
├── toon.rs              # TOON format encoding
├── risk.rs              # Behavioral risk calculation
├── error.rs             # Error types and exit codes
├── cache.rs             # Cache management and querying
├── shard.rs             # Sharded index generation
├── detectors/           # Language-specific extractors
│   ├── javascript/      # JS/TS with framework support
│   │   ├── core.rs      # Core JS/TS extraction
│   │   └── frameworks/  # React, Next.js, Express, Angular, Vue
│   ├── rust.rs
│   ├── python.rs
│   ├── go.rs
│   ├── java.rs
│   ├── kotlin.rs
│   ├── shell.rs
│   ├── gradle.rs
│   ├── c_family.rs
│   ├── markup.rs
│   ├── config.rs
│   ├── grammar.rs       # AST node mappings per language
│   └── generic.rs       # Generic extraction using grammars
├── mcp_server/          # MCP server (semfora-engine-server binary)
│   ├── mod.rs           # MCP tool handlers
│   └── bin.rs           # Server entry point
└── socket_server/       # WebSocket daemon (semfora-daemon binary)
    ├── mod.rs           # Server architecture
    ├── bin.rs           # Daemon entry point
    ├── connection.rs    # Client connection handling
    ├── protocol.rs      # Message types
    └── repo_registry.rs # Multi-repo context management
```

## Adding a New Language

1. Add tree-sitter grammar to `Cargo.toml`
2. Add `Lang` variant in `lang.rs` with extension mapping
3. Add `LangGrammar` in `detectors/grammar.rs` with AST node mappings
4. (Optional) Create dedicated detector in `detectors/` for special features
5. Wire up in `extract.rs` dispatcher

## Documentation

| Document | Description |
|----------|-------------|
| [Quick Start](docs/quickstart.md) | Get up and running in 5 minutes |
| [CLI Reference](docs/cli.md) | Complete CLI usage, flags, and examples |
| [Features](docs/features.md) | Incremental indexing, layered indexes, risk assessment |
| [WebSocket Daemon](docs/websocket-daemon.md) | Real-time updates, protocol, and query methods |
| [Adding Languages](docs/adding-languages.md) | Guide for adding new language support |
| [Engineering](docs/engineering.md) | Implementation details and status |

## License
