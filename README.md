# Semfora Engine

Semantic code analyzer that produces compressed TOON (Text Object-Oriented Notation) output for AI-assisted code review. Extracts symbols, dependencies, control flow, state changes, and risk assessments from source files.

## Installation

```bash
cargo build --release
```

## Usage

```bash
# Analyze a single file
semfora-mcp path/to/file.rs

# Start MCP server
semfora-mcp-server
```

## Supported Languages

### Programming Languages

| Language | Extensions | Family | Implementation Details |
|----------|------------|--------|------------------------|
| **TypeScript** | `.ts`, `.mts`, `.cts` | JavaScript | Full AST extraction via `tree-sitter-typescript`; exports, interfaces, enums, decorators |
| **TSX** | `.tsx` | JavaScript | TypeScript + JSX/React component detection, hooks, styled-components |
| **JavaScript** | `.js`, `.mjs`, `.cjs` | JavaScript | Functions, classes, imports; framework detection for React/Express/Angular |
| **JSX** | `.jsx` | JavaScript | JavaScript + JSX component detection |
| **Rust** | `.rs` | Rust | Functions, structs, traits, enums; `pub` visibility detection via `tree-sitter-rust` |
| **Python** | `.py`, `.pyi` | Python | Functions, classes, decorators; underscore-prefix privacy convention |
| **Go** | `.go` | Go | Functions, methods, structs; uppercase-export convention via `tree-sitter-go` |
| **Java** | `.java` | Java | Classes, interfaces, enums, methods; public/private/protected modifiers |
| **Kotlin** | `.kt`, `.kts` | Kotlin | Classes, functions, objects; visibility modifiers via `tree-sitter-kotlin-ng` |
| **C** | `.c`, `.h` | C Family | Functions, structs, enums; `extern` detection via `tree-sitter-c` |
| **C++** | `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`, `.hh` | C Family | Classes, structs, templates; access specifiers via `tree-sitter-cpp` |
| **Shell/Bash** | `.sh`, `.bash`, `.zsh`, `.fish` | Shell | Function definitions, variable assignments, command calls via `tree-sitter-bash` |
| **Gradle** | `.gradle` | Gradle | Groovy-based build files; closures, method calls via `tree-sitter-groovy` |

### Framework Detection (JavaScript Family)

| Framework | Detection Method | Extracted Information |
|-----------|------------------|----------------------|
| **React** | Import from `react` | Components, hooks (useState, useEffect, etc.), forwardRef, memo |
| **Next.js** | File path patterns (`/app/`, `/pages/`) | API routes, layouts, pages, server/client components |
| **Express** | Import from `express` | Route handlers (GET, POST, etc.), middleware |
| **Angular** | `@Component`, `@Injectable` decorators | Components, services, modules |
| **Vue** | `.vue` files, composition API | SFC script extraction, Options API, Composition API, Pinia stores |

### Markup & Styling

| Language | Extensions | Implementation Details |
|----------|------------|------------------------|
| **HTML** | `.html`, `.htm` | Document structure via `tree-sitter-html` |
| **CSS** | `.css` | Stylesheet detection via `tree-sitter-css` |
| **SCSS/SASS** | `.scss`, `.sass` | Stylesheet detection via `tree-sitter-scss` |
| **Markdown** | `.md`, `.markdown` | Document structure via `tree-sitter-md` |

### Configuration & Data

| Language | Extensions | Implementation Details |
|----------|------------|------------------------|
| **JSON** | `.json` | Structure parsing via `tree-sitter-json` |
| **YAML** | `.yaml`, `.yml` | Structure parsing via `tree-sitter-yaml` |
| **TOML** | `.toml` | Structure parsing via `tree-sitter-toml-ng` |
| **XML** | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.svg`, `.plist`, `.pom` | Structure parsing via `tree-sitter-xml` |
| **HCL/Terraform** | `.tf`, `.hcl`, `.tfvars` | Infrastructure-as-code via `tree-sitter-hcl` |

### Single-File Components

| Format | Extension | Implementation Details |
|--------|-----------|------------------------|
| **Vue SFC** | `.vue` | Extracts `<script>` or `<script setup>` section; detects `lang` attribute (ts/tsx/js); parses with appropriate grammar |

## Known Unsupported Formats

These formats were identified in test repositories but are not currently supported:

| Format | Extensions | Count* | Reason |
|--------|------------|--------|--------|
| **Jest Snapshots** | `.shot` | 5,140 | Test artifacts, not semantic code |
| **MDX** | `.mdx` | 861 | Documentation format (Markdown + JSX) |
| **AsciiDoc** | `.adoc` | 690 | Documentation format |
| **Protocol Buffers** | `.proto`, `.pb` | 550 | `devgen-tree-sitter-protobuf` requires tree-sitter 0.21 (incompatible) |
| **Ruby** | `.rb` | varies | No tree-sitter grammar added yet |
| **Swift** | `.swift` | varies | No tree-sitter grammar added yet |
| **PHP** | `.php` | varies | No tree-sitter grammar added yet |
| **Scala** | `.scala` | varies | No tree-sitter grammar added yet |
| **Elixir** | `.ex`, `.exs` | varies | No tree-sitter grammar added yet |

*Counts from typescript-eslint, terraform, spring-framework, and prometheus test repositories.

## Architecture

```
src/
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
├── lang.rs              # Language detection from file extensions
├── extract.rs           # Main extraction orchestration
├── schema.rs            # SemanticSummary output schema
├── toon.rs              # TOON format encoding
└── mcp_server/          # MCP server implementation
```

## Adding a New Language

1. Add tree-sitter grammar to `Cargo.toml`
2. Add `Lang` variant in `lang.rs` with extension mapping
3. Add `LangGrammar` in `detectors/grammar.rs` with AST node mappings
4. (Optional) Create dedicated detector in `detectors/` for special features
5. Wire up in `extract.rs` dispatcher

## License

MIT
