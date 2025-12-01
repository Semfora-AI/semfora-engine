MCP Semantic Diff & TOON Encoder
Local Deterministic Code Review Preprocessor

Language: Rust
Output Formats: TOON, JSON
Execution Mode: Local CLI binary
AI Usage: None in Phase 1
Primary Goal: Deterministically generate semantic TOON summaries from source files and diffs without any AI calls.

1. Motivation and Goals

Modern AI code review systems fail at scale because:

Raw diffs are token-inefficient

Large files exceed context budgets

Important semantic changes are buried in markup

Multi-language stacks (e.g., Tauri) fracture context

This project builds a compiler-grade semantic preprocessor that:

Converts source code and diffs into lossless semantic records

Encodes those records into TOON (Token-Oriented Object Notation)

Preserves all review-critical information

Discards presentation noise

Operates fully deterministically

Runs locally as a system binary

Requires zero AI calls in Phase 1

This serves as the front-end substrate for future MCP + AI review systems.

2. Core Technologies

Language: Rust

Parsing: tree-sitter

Diffing: similar

Encoding: TOON

Version Control Integration: Git

No network access, no model calls, no cloud dependency.

3. Phase 1 Scope (MVP)
Objective

From any directory, run:

mcp-diff layout.tsx


And receive:

A deterministic semantic summary

Encoded as TOON

For human inspection

With no AI calls

The tool must:

Accept any supported language

Parse using tree-sitter

Extract symbol, props, arguments, imports, inserts, and risk

Output TOON only

4. Phase 1 CLI Contract
Command
mcp-diff <path-to-file>

Behavior

Resolve absolute path

Detect language by extension

Load file contents

Parse with tree-sitter

Extract semantic surfaces

Generate deterministic summary

Encode in TOON

Print to STDOUT

Exit Codes
Code	Meaning
0	Success
1	File not found
2	Unsupported language
3	Parse failure
4	Internal semantic extraction failure
5. Deterministic Semantic Model (Lossless)

Every summary must preserve:

Symbol name

Symbol kind (function, component, class, method)

Arguments / props

Defaults

Return type

Imports / dependencies

State surfaces

Side effects

Structural insertions

Control flow changes

Public surface changes

Canonical Internal JSON Schema (Pre-TOON)
{
  "file": "layout.tsx",
  "language": "tsx",
  "symbol": "AppLayout",
  "symbol_kind": "component",
  "props": [],
  "arguments": [],
  "return_type": "JSX.Element",
  "insertions": [
    "header container with nav",
    "6 route links",
    "account dropdown with 2 routes + signout",
    "local open state via useState"
  ],
  "added_dependencies": ["useState", "Link"],
  "state_changes": [
    {
      "name": "open",
      "type": "boolean",
      "initializer": "false"
    }
  ],
  "control_flow_changes": [],
  "public_surface_changed": false,
  "behavioral_risk": "medium"
}


This structure is the single source of truth.
TOON is a lossless encoding layer only.

6. TOON Encoding Rules

Objects → indented blocks

Uniform arrays → tabular blocks

Strings quoted only if necessary

Field headers emitted once per array

Stable field ordering enforced

Phase 1 TOON Output Example
file: layout.tsx
language: tsx
symbol: AppLayout
symbol_kind: component
return_type: JSX.Element
public_surface_changed: false
behavioral_risk: medium

insertions[4]:
  header container with nav
  6 route links
  account dropdown with 2 routes + signout
  local open state via useState

added_dependencies[2]: useState,Link

state_changes[1]{name,type,initializer}:
  open,boolean,false

7. Language-Agnostic Semantic Extraction

All extraction is done using:

Tree-sitter grammar queries only

No type checker required in Phase 1

No AST mutation required

Universal Detectors
Detector	Purpose
Symbol detector	Finds nearest function/class/component
Import detector	Extracts new imports
Call detector	Finds call expressions
State detector	Finds variable initializers
Control detector	Finds if/loop/switch
JSX detector	Finds layout structures
API surface detector	Finds public exports

All detectors operate on AST node type + position only.

8. Insertion Summary Generation (Rule-Based)

Insertion summaries are generated via pattern rules, never by free-form text.

Examples:

if jsx_tag("header") → "header container"
if count(jsx_tag("Link")) >= 3 → "{N} route links"
if nested_button_menu → "account dropdown"
if call("useState") → "local open state via useState"
if call("useReducer") → "local reducer state"
if call("fetch") or call("invoke") → "network call introduced"


This ensures:

Determinism

Stable output

No hallucination

No variability between runs

9. Behavioral Risk Heuristic

Risk is computed numerically and mapped to buckets.

Point System
Change	Points
New import	+1
New state	+1
Control flow change	+2
New I/O or network	+2
Public API change	+3
Persistence	+3
Mapping
Score	Risk
0–1	low
2–3	medium
4+	high
10. Example Phase 1 Input and Output
Input File: layout.tsx
import { Outlet } from "react-router-dom";
import { useState } from "react";
import { Link } from "react-router-dom";

export default function AppLayout() {
  const [open, setOpen] = useState(false);

  return (
    <div>
      <header>
        <nav>
          <Link to="/a" />
          <Link to="/b" />
          <Link to="/c" />
          <Link to="/d" />
          <Link to="/e" />
          <Link to="/f" />
          <button onClick={() => setOpen(!open)}>Account</button>
          {open && <div>Sign out</div>}
        </nav>
      </header>
      <Outlet />
    </div>
  );
}

CLI Invocation
mcp-diff layout.tsx

Output (TOON)
file: layout.tsx
language: tsx
symbol: AppLayout
symbol_kind: component
return_type: JSX.Element
public_surface_changed: false
behavioral_risk: medium

insertions[4]:
  header container with nav
  6 route links
  account dropdown with sign out
  local open state via useState

added_dependencies[2]: useState,Link

state_changes[1]{name,type,initializer}:
  open,boolean,false

11. Safety Rules (No Silent Loss)

The encoder must not emit a summary unless all of the following are verified:

Symbol name is known

Symbol kind is known

All arguments or props are captured

All imports are captured

All state variables are captured

All call expressions are captured

All control flow deltas are captured

If any condition fails:

The tool emits:

A partial TOON output

Plus a RAW BLOCK fallback section with the localized source

This guarantees zero review-critical information loss.

12. Phase 2 (Diff-Aware Semantic Compression)

Adds:

git diff ingestion

Use of similar

Per-symbol change grouping

Before/after comparison generation

Call-site arity tracking

Signature delta detection

Public API surface change detection

New CLI:

mcp-diff --diff main..HEAD

13. Phase 3 (Cross-Language & Multi-Repo)

Adds:

Multiple repository indexing

Cross-repo call bridging

Tauri invoke(...) → Rust command mapping

OpenAPI / DTO boundary tracking

SDK ripple detection

14. Phase 4 (MCP Integration, Still Optional)

Adds:

MCP tool server

Streaming TOON payload delivery

Patch-only fix application hooks

Human gate vs auto-merge modes

AI becomes an optional consumer, not a dependency.

15. Non-Goals

This system explicitly does not:

Perform fuzzy natural language summarization

Perform type checking in Phase 1

Execute code

Modify files in Phase 1

Call any AI models in Phase 1

16. Project Structure (Rust)
mcp-diff/
  src/
    main.rs
    cli.rs
    file_loader.rs
    language_detect.rs
    parser.rs
    detectors/
      symbol.rs
      imports.rs
      state.rs
      calls.rs
      control.rs
      jsx.rs
    risk.rs
    schema.rs
    toon_encoder.rs
  grammars/
  Cargo.toml
  DESIGN.md

17. Why This Design Is Correct

Deterministic

Reproducible

Language-agnostic

Token-efficient

Lossless for review

Human-inspectable

AI-ready but not AI-dependent

Scales to very large files

Scales across multiple codebases

Works for mixed Rust + TypeScript stacks

18. Final Guarantee

This system never replaces code review.
It compresses code into a review-grade semantic representation.
It guarantees that no props, arguments, side effects, or public APIs are lost.
It guarantees that any failure to extract structure is surfaced, not hidden.
