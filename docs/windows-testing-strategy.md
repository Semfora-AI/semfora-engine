# Windows Testing Strategy for Semfora-Engine

## Executive Summary

This document analyzes approaches for testing Windows functionality of semfora-engine from a Linux development environment using Docker containers. The key insight is that **native Windows cannot run in Docker on Linux** - Docker containers share the host kernel. However, there are several effective strategies for cross-platform testing.

---

## Current State Analysis

### Platform-Specific Code Inventory

The following areas have Windows-specific logic:

| Location | Concern | Risk Level |
|----------|---------|------------|
| `src/installer/platform/mod.rs:75-77` | `#[cfg(target_os = "windows")]` platform detection | Medium |
| `src/installer/platform/paths.rs:174-194` | Windows path resolution (LOCALAPPDATA) | High |
| `src/installer/platform/paths.rs:200-207` | PATH separator (`;` vs `:`) | Medium |
| `src/installer/platform/mod.rs:96-100` | Binary extension (`.exe` vs empty) | Low |

### Current CI Coverage

```yaml
# From .github/workflows/ci.yml
matrix:
  - os: windows-latest        # Builds on Windows
    target: x86_64-pc-windows-msvc

# But tests only run on Linux:
- name: Run tests
  if: matrix.os == 'ubuntu-latest'  # <-- Tests skipped on Windows!
```

**Gap identified:** Windows builds but doesn't test in CI.

---

## Why Docker Won't Solve This Directly

### Technical Constraints

1. **Kernel Sharing**: Docker containers share the host OS kernel. A Linux host can only run Linux containers.

2. **Windows Containers**: Exist but require:
   - Windows Server host or Windows 10/11 Pro with Hyper-V
   - `--platform=windows` flag
   - Cannot run on Linux hosts

3. **QEMU/VM Emulation**: Full Windows VM emulation is possible but:
   - Extremely slow (10-100x slower)
   - Heavy resource requirements
   - Not practical for CI/development iteration

---

## Viable Testing Strategies

### Strategy 1: Cross-Compilation + WINE Testing (Recommended for Local Dev)

**Concept:** Build Windows binaries using cross-compilation, then test them with WINE.

```dockerfile
# Dockerfile.windows-test
FROM rust:1.83-slim

# Install cross-compilation toolchain and WINE
RUN dpkg --add-architecture i386 && \
    apt-get update && apt-get install -y \
    mingw-w64 \
    wine64 \
    wine32 \
    xvfb \
    && rm -rf /var/lib/apt/lists/*

# Add Windows target
RUN rustup target add x86_64-pc-windows-gnu

# Set up WINE prefix
ENV WINEPREFIX=/root/.wine
ENV WINEDEBUG=-all

WORKDIR /app
```

**Usage:**
```bash
# Build for Windows (cross-compile)
docker run -v $(pwd):/app windows-test \
  cargo build --release --target x86_64-pc-windows-gnu

# Run tests under WINE
docker run -v $(pwd):/app windows-test \
  wine64 target/x86_64-pc-windows-gnu/release/semfora-engine.exe --version
```

**Limitations:**
- WINE isn't perfect Windows emulation
- Some Win32 API calls may behave differently
- File system paths still use Unix conventions unless explicitly tested

**Best for:** Quick smoke tests, checking binary runs at all

---

### Strategy 2: Mock-Based Unit Testing (Recommended for CI)

**Concept:** Abstract platform-specific code behind traits, test with mocks.

```rust
// src/installer/platform/mod.rs

/// Trait for platform-specific operations
pub trait PlatformProvider {
    fn home_dir(&self) -> Option<PathBuf>;
    fn config_dir(&self) -> Option<PathBuf>;
    fn binary_extension(&self) -> &'static str;
    fn path_separator(&self) -> char;
    fn env_var(&self, key: &str) -> Option<String>;
}

/// Real implementation (production)
pub struct NativePlatform;

impl PlatformProvider for NativePlatform {
    // Uses std::env, dirs crate, etc.
}

/// Mock implementation (testing)
#[cfg(test)]
pub struct MockWindowsPlatform {
    pub local_appdata: PathBuf,
    pub path: String,
}

#[cfg(test)]
impl PlatformProvider for MockWindowsPlatform {
    fn home_dir(&self) -> Option<PathBuf> {
        Some(PathBuf::from("C:\\Users\\TestUser"))
    }

    fn config_dir(&self) -> Option<PathBuf> {
        Some(self.local_appdata.join("semfora"))
    }

    fn binary_extension(&self) -> &'static str {
        ".exe"
    }

    fn path_separator(&self) -> char {
        ';'
    }

    fn env_var(&self, key: &str) -> Option<String> {
        match key {
            "LOCALAPPDATA" => Some(self.local_appdata.to_string_lossy().into()),
            "PATH" => Some(self.path.clone()),
            _ => None,
        }
    }
}
```

**Test Example:**
```rust
#[test]
fn test_windows_path_resolution() {
    let mock = MockWindowsPlatform {
        local_appdata: PathBuf::from("C:\\Users\\Test\\AppData\\Local"),
        path: "C:\\Windows;C:\\Users\\Test\\AppData\\Local\\semfora\\bin".into(),
    };

    let paths = InstallerPaths::from_provider(&mock);

    assert_eq!(
        paths.config_file,
        PathBuf::from("C:\\Users\\Test\\AppData\\Local\\semfora\\config.toml")
    );
    assert_eq!(
        paths.engine_binary,
        PathBuf::from("C:\\Users\\Test\\AppData\\Local\\semfora\\bin\\semfora-engine.exe")
    );
    assert!(paths.is_in_path(&mock)); // PATH contains binary_dir
}
```

**Benefits:**
- Runs on any platform
- Fast execution
- Tests logic, not syscalls
- Can simulate edge cases (missing env vars, permissions)

---

### Strategy 3: GitHub Actions Windows Runner (Current)

**Improve existing CI** to actually run tests on Windows:

```yaml
# .github/workflows/ci.yml

- name: Run tests
  run: cargo test --release --all-targets
  env:
    RUST_BACKTRACE: 1
  # Remove the Linux-only condition:
  # if: matrix.os == 'ubuntu-latest'  <-- DELETE THIS
```

**Additional Windows-specific tests:**
```yaml
- name: Run Windows integration tests
  if: matrix.os == 'windows-latest'
  run: |
    # Test PATH manipulation
    $env:PATH += ";$pwd\target\${{ matrix.target }}\release"
    semfora-engine.exe --version

    # Test installer paths
    cargo test --release installer::platform -- --nocapture
```

**Benefits:**
- Actual Windows environment
- Tests real Win32 API calls
- Free for open source projects

**Costs:**
- Slower than Linux runners (~2-3x)
- More expensive for private repos

---

### Strategy 4: Docker + Remote Windows Agent

**Concept:** Use Docker for Linux testing, remote Windows machine for Windows testing.

```
┌─────────────────────────────────────────┐
│  Local Development (Linux)              │
│  ┌───────────────────────────────────┐  │
│  │  Docker Container                 │  │
│  │  - Unit tests                     │  │
│  │  - Linux integration tests        │  │
│  │  - Cross-compile to Windows       │  │
│  └───────────────────────────────────┘  │
│              │                          │
│              │ SSH/WinRM                │
│              ▼                          │
│  ┌───────────────────────────────────┐  │
│  │  Windows VM (VirtualBox/Hyper-V)  │  │
│  │  - Windows integration tests      │  │
│  │  - Installer testing              │  │
│  │  - PATH manipulation tests        │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

**Implementation with Vagrant:**
```ruby
# Vagrantfile
Vagrant.configure("2") do |config|
  config.vm.define "windows" do |win|
    win.vm.box = "gusztavvargadr/windows-11"
    win.vm.network "private_network", ip: "192.168.56.10"

    win.vm.provision "shell", inline: <<-SHELL
      # Install Rust
      Invoke-WebRequest -Uri https://win.rustup.rs -OutFile rustup-init.exe
      ./rustup-init.exe -y
    SHELL
  end
end
```

**Test runner script:**
```bash
#!/bin/bash
# scripts/test-windows.sh

# Cross-compile in Docker
docker run -v $(pwd):/app rust:1.83 \
  cargo build --release --target x86_64-pc-windows-gnu

# Copy to Windows VM and test
vagrant upload target/x86_64-pc-windows-gnu/release/*.exe windows:C:/test/
vagrant ssh windows -c "C:/test/semfora-engine.exe --version"
vagrant ssh windows -c "cargo test --release -- installer::platform"
```

---

### Strategy 5: Path Normalization Testing

**Concept:** Test path handling without needing Windows at all.

The key insight is that most "Windows bugs" are about:
1. Backslash vs forward slash
2. Drive letters (`C:`)
3. Case insensitivity
4. Path length limits

```rust
#[cfg(test)]
mod path_normalization_tests {
    use std::path::{Path, PathBuf};

    #[test]
    fn test_path_normalization_handles_windows_style() {
        // Our code should handle both styles
        let unix_style = PathBuf::from("/home/user/.config/semfora");
        let windows_style = PathBuf::from("C:\\Users\\User\\AppData\\Local\\semfora");

        // Test that path operations work regardless of style
        assert!(unix_style.join("config.toml").to_string_lossy().contains("config.toml"));
        assert!(windows_style.join("config.toml").to_string_lossy().contains("config.toml"));
    }

    #[test]
    fn test_path_display_uses_native_separators() {
        // std::path automatically handles this, but verify our code doesn't break it
        let path = PathBuf::from("C:/Users/Test/AppData"); // Forward slashes

        // On Windows, display would show backslashes
        // On Unix, this is just a weird path name
        // Our code should never hardcode separators
    }

    #[test]
    fn test_binary_extension_handling() {
        let base = PathBuf::from("semfora-engine");

        // Simulate Windows behavior
        let windows_binary = base.with_extension("exe");
        assert_eq!(windows_binary.file_name().unwrap(), "semfora-engine.exe");

        // Simulate Unix behavior (no extension)
        let unix_binary = base.clone();
        assert_eq!(unix_binary.file_name().unwrap(), "semfora-engine");
    }
}
```

---

## Recommended Implementation Plan

### Phase 1: Enable Windows CI Tests (Quick Win)

1. Remove `if: matrix.os == 'ubuntu-latest'` from CI workflow
2. Fix any test failures that appear on Windows
3. Add Windows-specific test attributes where needed:
   ```rust
   #[test]
   #[cfg(target_os = "windows")]
   fn test_windows_specific_behavior() { ... }
   ```

### Phase 2: Add Mock-Based Platform Testing

1. Create `PlatformProvider` trait
2. Implement `MockWindowsPlatform` and `MockUnixPlatform`
3. Refactor `InstallerPaths` to accept any `PlatformProvider`
4. Add comprehensive mock-based tests

### Phase 3: Local Docker Development Setup

1. Create `Dockerfile.dev` for development
2. Add WINE-based smoke tests
3. Document local testing workflow

### Phase 4: Integration Testing Infrastructure

1. Set up Vagrant Windows VM (optional)
2. Create `scripts/test-all-platforms.sh`
3. Add to `CONTRIBUTING.md`

---

## Docker Compose Configuration (Optional)

For a complete local development setup:

```yaml
# docker-compose.dev.yml
version: '3.8'

services:
  # Linux development and testing
  dev:
    build:
      context: .
      dockerfile: Dockerfile.dev
    volumes:
      - .:/app
      - cargo-cache:/usr/local/cargo/registry
    working_dir: /app
    command: cargo watch -x test

  # Cross-compilation to Windows
  cross-windows:
    build:
      context: .
      dockerfile: Dockerfile.windows-cross
    volumes:
      - .:/app
      - cargo-cache:/usr/local/cargo/registry
    working_dir: /app
    command: cargo build --target x86_64-pc-windows-gnu

  # WINE-based testing
  wine-test:
    build:
      context: .
      dockerfile: Dockerfile.wine
    volumes:
      - .:/app
    working_dir: /app
    depends_on:
      - cross-windows
    command: wine64 target/x86_64-pc-windows-gnu/release/semfora-engine.exe --version

volumes:
  cargo-cache:
```

---

## Conclusion

**The key takeaway:** Docker on Linux cannot run native Windows. Instead, use:

1. **Mock-based testing** for platform abstraction logic (recommended)
2. **GitHub Actions Windows runner** for real Windows testing (already available)
3. **WINE in Docker** for quick smoke tests
4. **Vagrant Windows VM** for comprehensive local testing (optional)

The most effective approach combines Strategy 2 (mocks) with Strategy 3 (GitHub Actions) to achieve comprehensive Windows coverage without significant infrastructure overhead.

---

## Appendix: Dockerfile Templates

### Dockerfile.dev (Linux Development)

```dockerfile
FROM rust:1.83-slim

RUN apt-get update && apt-get install -y \
    git \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-watch

WORKDIR /app
```

### Dockerfile.windows-cross (Cross-Compilation)

```dockerfile
FROM rust:1.83-slim

RUN apt-get update && apt-get install -y \
    gcc-mingw-w64-x86-64 \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-pc-windows-gnu

# Configure cargo for cross-compilation
RUN mkdir -p /root/.cargo && \
    echo '[target.x86_64-pc-windows-gnu]\nlinker = "x86_64-w64-mingw32-gcc"' > /root/.cargo/config.toml

WORKDIR /app
```

### Dockerfile.wine (WINE Testing)

```dockerfile
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN dpkg --add-architecture i386 && \
    apt-get update && apt-get install -y \
    wine64 \
    wine32 \
    xvfb \
    && rm -rf /var/lib/apt/lists/*

ENV WINEPREFIX=/root/.wine
ENV WINEDEBUG=-all
ENV DISPLAY=:99

# Initialize WINE
RUN Xvfb :99 -screen 0 1024x768x16 & \
    sleep 2 && \
    wineboot --init && \
    sleep 5

WORKDIR /app
```

---

## Appendix B: Comprehensive Filesystem Operations Audit

This section catalogs every filesystem operation in the codebase, categorized by type and risk level for cross-platform compatibility.

### Summary Statistics

| Category | Production Code | Test Code | Total |
|----------|-----------------|-----------|-------|
| File Reads | 67 | 48 | 115 |
| File Writes | 31 | 52 | 83 |
| Directory Operations | 24 | 28 | 52 |
| Permission Checks | 2 | 0 | 2 |
| Path Canonicalization | 6 | 0 | 6 |

### Critical Issues Identified

#### 1. Cache Base Directory - **BROKEN ON WINDOWS**

**Location:** `src/cache.rs:2342-2355`

```rust
pub fn get_cache_base_dir() -> PathBuf {
    // Check XDG_CACHE_HOME first
    if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(xdg_cache).join("semfora");
    }

    // Fall back to ~/.cache/semfora  <-- UNIX ONLY!
    if let Some(home) = dirs::home_dir() {
        return home.join(".cache").join("semfora");
    }

    // Last resort: temp directory
    std::env::temp_dir().join("semfora")
}
```

**Problem:** On Windows, this creates `C:\Users\<user>\.cache\semfora` instead of the proper `%LOCALAPPDATA%\semfora\cache`.

**Fix Required:** Add Windows-specific path:
```rust
#[cfg(windows)]
{
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local_appdata).join("semfora").join("cache");
    }
}
```

#### 2. Permission Check - **WRONG SEMANTICS ON WINDOWS**

**Location:** `src/installer/platform/paths.rs:127-128, 153-155`

```rust
let binary_dir = if std::fs::metadata("/usr/local/bin")
    .map(|m| m.permissions().readonly())
    .unwrap_or(true)
```

**Problem:** `readonly()` on Windows only checks `FILE_ATTRIBUTE_READONLY`, not actual write permissions (ACLs).

**Impact:** May install to wrong directory on Windows or fail to detect writable locations.

---

### Production Code - File Reads

#### Core Cache System (`src/cache.rs`)

| Line | Operation | Purpose | Windows Risk |
|------|-----------|---------|--------------|
| 393 | `fs::read_to_string(path)` | Read branch SHA | Low |
| 414 | `fs::read_to_string(path)` | Read repo hash | Low |
| 615 | `fs::read_to_string(&path)` | Load cache metadata | Low |
| 662 | `fs::read_to_string(&path)` | Load module data | Low |
| 816-817 | `fs::File::open` + `BufReader` | Load symbol index | Low |
| 891-892 | `fs::File::open` + `BufReader` | Query symbol index | Low |
| 970-971 | `fs::File::open` + `BufReader` | Load symbol entries | Low |
| 1027-1028 | `fs::File::open` + `BufReader` | List symbol entries | Low |
| 1382 | `fs::read_to_string(&meta_path)` | Load layer metadata | Low |
| 1416 | `fs::read_to_string(&deleted_path)` | Load deleted files | Low |
| 1694 | `std::fs::read_to_string(path)` | Read source file | Low |

#### MCP Server (`src/mcp_server/`)

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| mod.rs | 247 | `fs::read_to_string(&overview_path)` | Low |
| mod.rs | 308 | `fs::read_to_string(&module_path)` | Low |
| mod.rs | 367 | `fs::read_to_string(&resolved_path)` | Low |
| mod.rs | 477 | `fs::read_to_string(&overview_path)` | Low |
| mod.rs | 579, 616, 639 | `fs::read_to_string(&symbol_path)` | Low |
| mod.rs | 822-834 | `fs::File::open` + `BufReader` | Low |
| mod.rs | 1099, 1857 | `fs::read_to_string(&file_path)` | Low |
| mod.rs | 2124 | `fs::read_to_string(&call_graph_path)` | Low |
| mod.rs | 2253 | `fs::read_to_string(&full_path)` | Low |
| helpers.rs | 336 | `fs::read_to_string(file_path)` | Low |
| helpers.rs | 1081 | `fs::read_to_string(&sig_path)` | Low |
| helpers.rs | 1106 | `fs::read_to_string(&call_graph_path)` | Low |
| formatting.rs | 34, 81, 158, 240 | Various `fs::read_to_string` | Low |
| formatting.rs | 576-578 | `fs::File::open` + `BufReader` | Low |

#### Installer (`src/installer/`)

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| config.rs | 109 | `fs::read_to_string(path)` | Low |
| clients/mod.rs | 206 | `fs::read_to_string(path)` | Low |
| clients/vscode.rs | 19 | `fs::read_to_string(path)` | Low |
| clients/custom.rs | 153 | `std::fs::read_to_string(&path)` | Low |

#### Commands (`src/commands/`)

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| index.rs | 58, 194 | `fs::read_to_string(&meta_path)` | Low |
| index.rs | 360 | `fs::read_to_string(file_path)` | Low |
| analyze.rs | 104, 201, 335, 413, 458 | `fs::read_to_string` | Low |
| query.rs | 62, 429, 944 | `fs::read_to_string` | Low |
| toon_parser.rs | 237, 260 | `fs::read_to_string(path)` | Low |
| commit.rs | 30, 295 | `fs::read_to_string` | Low |
| search.rs | 692 | `std::fs::read_to_string(&file_path)` | Low |
| security.rs | 49-50 | `fs::File::open` + `BufReader` | Low |
| validate.rs | 45-46 | `fs::File::open` + `BufReader` | Low |

#### Other Modules

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| shard.rs | N/A | Reads via BufReader | Low |
| bm25.rs | 264 | `std::fs::read_to_string(path)` | Low |
| server/sync.rs | 270, 583-586 | `std::fs::read_to_string`, `File::open` | Low |
| socket_server/indexer.rs | 88 | `fs::read_to_string(file_path)` | Low |
| socket_server/connection.rs | 240, 321 | `std::fs::read_to_string` | Low |
| security/patterns/embedded.rs | 90, 220 | `std::fs::read(path)` | Low |
| security/compiler/bin.rs | 246, 294 | `std::fs::read(&input)` | Low |
| benchmark.rs | 269 | `fs::read_to_string(file_path)` | Low |
| test_runner.rs | 170 | `std::fs::read_to_string` | Low |

---

### Production Code - File Writes

#### Core Cache System (`src/cache.rs`)

| Line | Operation | Purpose | Windows Risk |
|------|-----------|---------|--------------|
| 402 | `fs::write(&path, sha)` | Write branch SHA | Low |
| 423 | `fs::write(&path, hash)` | Write repo hash | Low |
| 849-861 | `fs::File::create` + `fs::rename` | Atomic symbol index write | **Medium** |
| 1313-1359 | Multiple atomic writes | Layer persistence | **Medium** |
| 1484 | `fs::write(self.layer_meta_path(), ...)` | Layer metadata | Low |
| 1780-1788 | `std::fs::write` | Graph files | Low |

**Windows Risk (Medium):** `fs::rename` fails if target exists on Windows. Need `MOVEFILE_REPLACE_EXISTING`.

#### Shard Writer (`src/shard.rs`)

| Line | Operation | Purpose | Windows Risk |
|------|-----------|---------|--------------|
| 546-547 | `fs::File::create` + `write_all` | Write overview | Low |
| 566-567 | `fs::File::create` + `write_all` | Write modules | Low |
| 592-604 | `fs::File::create` + `write_all` | Write symbols | Low |
| 620 | `fs::write` | Call graph | Low |
| 626 | `fs::write` | Import graph | Low |
| 632 | `fs::write` | Module graph | Low |
| 646-647 | `fs::File::create` + `write_all` | Signature index | Low |
| 748 | `fs::File::create` | BM25 index | Low |

#### MCP Server (`src/mcp_server/helpers.rs`)

| Line | Operation | Purpose | Windows Risk |
|------|-----------|---------|--------------|
| 519 | `fs::remove_file(module_path)` | Delete stale module | Low |
| 528 | `fs::write(&module_path, toon)` | Write module | Low |
| 554 | `fs::OpenOptions::new()` | Touch file | Low |
| 584, 593 | `fs::write(&path, toon)` | Write symbol files | Low |

#### Installer (`src/installer/`)

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| config.rs | 130 | `fs::create_dir_all(parent)` | Low |
| config.rs | 142 | `fs::write(&temp_path, ...)` | Low |
| config.rs | 147 | `fs::rename(&temp_path, path)` | **Medium** |
| clients/mod.rs | 226 | `fs::create_dir_all(parent)` | Low |
| clients/mod.rs | 238 | `fs::write(&temp_path, ...)` | Low |
| clients/mod.rs | 244 | `fs::rename(&temp_path, path)` | **Medium** |
| clients/mod.rs | 259 | `fs::copy(path, &backup_path)` | Low |
| clients/vscode.rs | 38, 49, 54 | create_dir, write, rename | **Medium** |
| clients/vscode.rs | 161, 198, 214 | `fs::copy` | Low |
| uninstall.rs | 242 | `fs::remove_dir_all(&paths.cache_dir)` | Low |
| uninstall.rs | 262 | `fs::remove_file(&paths.config_file)` | Low |

#### Other Modules

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| bm25.rs | 259 | `std::fs::write(path, json)` | Low |
| sqlite_export.rs | 93, 98 | `fs::create_dir_all`, `fs::remove_file` | Low |
| security/compiler/bin.rs | 207, 237 | `std::fs::write(&output, ...)` | Low |
| security/compiler/mod.rs | 283 | `std::fs::write(path, bytes)` | Low |
| benchmark_builder/generator.rs | 17-43 | `fs::create_dir_all`, `fs::write` | Low |
| benchmark_builder/runner.rs | 48-185 | Multiple `fs::write` | Low |

---

### Production Code - Directory Operations

#### Cache System (`src/cache.rs`)

| Line | Operation | Purpose | Windows Risk |
|------|-----------|---------|--------------|
| 276-281 | `fs::create_dir_all` (6x) | Initialize cache structure | Low |
| 541-544 | `fs::create_dir_all` | Layer directories | Low |
| 554 | `fs::read_dir(self.modules_dir())` | List modules | Low |
| 573 | `fs::read_dir(self.symbols_dir())` | List symbols | Low |
| 598 | `fs::remove_dir_all(&self.root)` | Clear cache | Low |
| 1296 | `fs::create_dir_all(&layer_dir)` | Create layer dir | Low |
| 1520 | `fs::remove_dir_all(&layers_dir)` | Clear layers | Low |
| 1764 | `std::fs::create_dir_all(&graphs_dir)` | Graphs directory | Low |
| 2407 | `fs::read_dir(path)` | Directory size | Low |
| 2426, 2449 | `fs::read_dir(&cache_base)` | List caches | Low |
| 2460 | `fs::remove_dir_all(&path)` | Prune old cache | Low |

#### Other Modules

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| commands/index.rs | 324 | `fs::read_dir(dir)` | Low |
| commands/analyze.rs | 607 | `fs::read_dir(dir)` | Low |
| commands/query.rs | 875, 907 | `fs::read_dir(&modules_dir)` | Low |
| mcp_server/helpers.rs | 185, 246 | `fs::read_dir(dir)` | Low |
| benchmark.rs | 321 | `fs::read_dir(dir)` | Low |
| test_runner.rs | 196 | `std::fs::read_dir(dir)` | Low |
| socket_server/indexer.rs | 156 | `fs::read_dir(dir)` | Low |
| benchmark_builder/snapshot.rs | 322 | `fs::read_dir(dir)` | Low |

---

### Production Code - Metadata & Permission Checks

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| installer/platform/paths.rs | 127-128 | `fs::metadata().permissions().readonly()` | **HIGH** |
| installer/platform/paths.rs | 153-155 | `fs::metadata().permissions().readonly()` | **HIGH** |
| cache.rs | 102, 126 | `fs::metadata(path)` | Low |
| cache.rs | 1601 | `fs::metadata(&full_path)` | Low |
| mcp_server/helpers.rs | 60, 109, 208 | `fs::metadata` | Low |
| mcp_server/mod.rs | 822 | `fs::metadata(&call_graph_path)` | Low |
| shard.rs | 737, 831, 918 | `fs::metadata(&path)?.len()` | Low |
| sqlite_export.rs | 164 | `fs::metadata(output_path)` | Low |

---

### Production Code - Path Canonicalization

| File | Line | Operation | Windows Risk |
|------|------|-----------|--------------|
| cache.rs | 245 | `repo_path.canonicalize()` | **Medium** |
| cache.rs | 260 | `worktree_path.canonicalize()` | **Medium** |
| cache.rs | 2369 | `.canonicalize()` | **Medium** |
| commands/analyze.rs | 287 | `dir_path.canonicalize()` | **Medium** |
| socket_server/repo_registry.rs | 152-153 | `.canonicalize()` | **Medium** |

**Windows Risk (Medium):** `canonicalize()` resolves symlinks and returns `\\?\` prefixed paths on Windows, which may cause issues with path comparison.

---

### Configurable Path System

#### Current Configuration Structure (`src/installer/config.rs`)

```rust
pub struct SemforaConfig {
    pub cache: CacheConfig,      // cache.dir: Option<PathBuf>
    pub logging: LoggingConfig,
    pub mcp: McpConfig,
    pub patterns: PatternConfig, // patterns.url: String
}
```

#### Cache Directory Resolution Chain

1. **CLI argument:** `--cache-dir <path>`
2. **Config file:** `cache.dir` in config.toml
3. **Environment:** `XDG_CACHE_HOME` (Unix) or should be `LOCALAPPDATA` (Windows)
4. **Default:** `~/.cache/semfora` (Unix) - **BROKEN on Windows**

#### Files That Need Configurable Path Support

| Component | Currently Configurable | Notes |
|-----------|----------------------|-------|
| Cache directory | Yes (partial) | Missing Windows default |
| Config file location | No | Hardcoded in SemforaPaths |
| Binary install directory | No | Determined by permission check |
| Log file location | No | Uses tracing-subscriber |
| Security patterns cache | Partial | URL configurable, local path hardcoded |

---

### Abstraction Recommendations

#### Proposed `FileSystem` Trait

```rust
/// Core filesystem operations - mockable for testing
pub trait FileSystem: Send + Sync {
    // Read operations
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn exists(&self, path: &Path) -> bool;
    fn metadata(&self, path: &Path) -> io::Result<Metadata>;

    // Write operations
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

    // Atomic operations (platform-specific behavior)
    fn atomic_write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    // Directory listing
    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>>;

    // Platform-specific
    fn is_dir_writable(&self, path: &Path) -> bool;
}

/// Platform-aware path resolution
pub trait PathResolver: Send + Sync {
    fn cache_base_dir(&self) -> PathBuf;
    fn config_dir(&self) -> PathBuf;
    fn home_dir(&self) -> Option<PathBuf>;
    fn temp_dir(&self) -> PathBuf;
}
```

#### Files Requiring Refactoring

**High Priority (use `FileSystem` trait):**
1. `src/cache.rs` - 50+ fs operations
2. `src/shard.rs` - Index writing
3. `src/mcp_server/mod.rs` - Cache reading
4. `src/installer/config.rs` - Config read/write

**Medium Priority:**
1. `src/commands/*.rs` - Various file reads
2. `src/mcp_server/helpers.rs` - File operations
3. `src/installer/clients/*.rs` - Config manipulation

**Can Remain Direct:**
1. Test files (`tests/**/*.rs`) - Test-specific setup
2. Benchmarks (`benches/*.rs`) - Performance testing
3. Examples (`examples/*.rs`) - Demo code

---

### Estimated Refactoring Effort

| Component | Files | FS Calls | Effort |
|-----------|-------|----------|--------|
| Cache system | 1 | ~50 | Large (2-3 days) |
| Shard writer | 1 | ~15 | Medium (1 day) |
| MCP server | 3 | ~30 | Medium (1-2 days) |
| Installer | 5 | ~20 | Medium (1 day) |
| Commands | 8 | ~25 | Medium (1-2 days) |
| **Total** | **18** | **~140** | **~7-10 days** |

### Quick Wins (No Trait Needed)

1. **Fix `get_cache_base_dir()`** - Add Windows LOCALAPPDATA support
2. **Fix permission check** - Replace `readonly()` with proper Windows ACL check or remove entirely for Windows (always use user directory)
3. **Enable Windows CI tests** - Remove the Linux-only gate

These three changes would fix the most critical Windows issues without requiring the full abstraction refactor.
