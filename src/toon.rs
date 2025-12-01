//! TOON (Token-Oriented Object Notation) encoder using rtoon library
//!
//! TOON encoding rules from specification:
//! - Objects -> indented blocks
//! - Uniform arrays -> tabular blocks
//! - Strings quoted only if necessary
//! - Field headers emitted once per array
//! - Stable field ordering enforced

use std::collections::HashMap;

use rtoon::encode_default;
use serde_json::{json, Map, Value};

use crate::schema::{ModuleGroup, RepoOverview, RepoStats, RiskLevel, SemanticSummary, SymbolKind};

// ============================================================================
// Noisy call filtering - these are implementation details, not architecture
// ============================================================================

/// Array/collection methods that are implementation noise
const NOISY_ARRAY_METHODS: &[&str] = &[
    "includes", "filter", "map", "reduce", "forEach", "find", "findIndex",
    "some", "every", "slice", "splice", "push", "pop", "shift", "unshift",
    "concat", "join", "sort", "reverse", "indexOf", "lastIndexOf", "flat",
    "flatMap", "fill", "copyWithin", "entries", "keys", "values", "at",
];

/// Promise chain methods - the actual logic inside is captured separately
const NOISY_PROMISE_METHODS: &[&str] = &[
    "then", "catch", "finally",
];

/// ORM/Schema builder methods - these are declarations, not runtime behavior
const NOISY_SCHEMA_METHODS: &[&str] = &[
    "notNull", "primaryKey", "default", "references", "unique", "index",
    "serial", "text", "integer", "bigint", "boolean", "timestamp", "jsonb",
    "varchar", "char", "numeric", "real", "double", "date", "time", "uuid",
];

/// Math methods that are implementation noise
const NOISY_MATH_METHODS: &[&str] = &[
    "floor", "ceil", "round", "random", "abs", "sqrt", "pow", "min", "max",
    "sin", "cos", "tan", "log", "exp",
];

/// String methods that are implementation noise
const NOISY_STRING_METHODS: &[&str] = &[
    "split", "trim", "toLowerCase", "toUpperCase", "substring", "substr",
    "charAt", "charCodeAt", "replace", "replaceAll", "match", "search",
    "startsWith", "endsWith", "padStart", "padEnd", "repeat",
];

/// Object methods that are implementation noise
const NOISY_OBJECT_METHODS: &[&str] = &[
    "keys", "values", "entries", "assign", "freeze", "seal",
    "hasOwnProperty", "toString", "valueOf",
];

/// HTTP methods for API calls
const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

/// API/HTTP client libraries
const API_CLIENT_NAMES: &[&str] = &[
    "axios", "fetch", "ky", "got", "superagent", "request", "invoke",
];

/// React Query / TanStack Query hooks
const REACT_QUERY_HOOKS: &[&str] = &[
    "useQuery", "useMutation", "useInfiniteQuery", "useQueries",
    "useSuspenseQuery", "useSuspenseInfiniteQuery", "usePrefetchQuery",
    "queryClient", "useQueryClient",
];

/// SWR hooks
const SWR_HOOKS: &[&str] = &[
    "useSWR", "useSWRMutation", "useSWRInfinite", "useSWRConfig",
];

/// Apollo GraphQL hooks
const APOLLO_HOOKS: &[&str] = &[
    "useApolloClient", "useLazyQuery", "useSubscription",
    "useReactiveVar", "useSuspenseQuery_experimental",
];

/// Check if a call is meaningful (not noise)
pub fn is_meaningful_call(name: &str, object: Option<&str>) -> bool {
    // Always keep React hooks (useState, useEffect, etc.)
    if name.starts_with("use") && name.chars().nth(3).map(|c| c.is_uppercase()).unwrap_or(false) {
        return true;
    }

    // Always keep state setters
    if name.starts_with("set") && name.chars().nth(3).map(|c| c.is_uppercase()).unwrap_or(false) {
        return true;
    }

    // React Query / TanStack Query
    if REACT_QUERY_HOOKS.contains(&name) {
        return true;
    }

    // SWR
    if SWR_HOOKS.contains(&name) {
        return true;
    }

    // Apollo GraphQL
    if APOLLO_HOOKS.contains(&name) {
        return true;
    }

    // Direct API client calls (fetch, axios, ky, etc.)
    if API_CLIENT_NAMES.contains(&name) {
        return true;
    }

    // HTTP methods on API clients (axios.get, ky.post, etc.)
    if let Some(obj) = object {
        if API_CLIENT_NAMES.contains(&obj) && HTTP_METHODS.contains(&name) {
            return true;
        }
    }

    // Always keep I/O and database calls
    if matches!(name, "insert" | "select" | "update" | "delete" | "query" | "execute" | "migrate" | "mutate") {
        return true;
    }

    // Filter promise chain methods (logic inside is captured separately)
    if NOISY_PROMISE_METHODS.contains(&name) {
        return false;
    }

    // Filter ORM/schema builder methods (declarations, not runtime)
    if NOISY_SCHEMA_METHODS.contains(&name) {
        return false;
    }

    // Filter based on object
    if let Some(obj) = object {
        // Math methods are noise
        if obj == "Math" && NOISY_MATH_METHODS.contains(&name) {
            return false;
        }

        // Object methods are noise
        if obj == "Object" && NOISY_OBJECT_METHODS.contains(&name) {
            return false;
        }

        // Array-like methods on data objects are noise
        if NOISY_ARRAY_METHODS.contains(&name) {
            return false;
        }

        // String methods are noise
        if NOISY_STRING_METHODS.contains(&name) {
            return false;
        }

        // Keep database, Response, process calls
        if matches!(obj, "db" | "Response" | "process" | "console" | "document" | "window") {
            return true;
        }
    }

    // Filter standalone noisy calls
    if NOISY_ARRAY_METHODS.contains(&name) {
        return false;
    }

    // Keep require, drizzle, postgres, etc.
    true
}

/// Filter calls to only meaningful ones
pub fn filter_meaningful_calls(calls: &[crate::schema::Call]) -> Vec<crate::schema::Call> {
    calls
        .iter()
        .filter(|c| is_meaningful_call(&c.name, c.object.as_deref()))
        .cloned()
        .collect()
}

// ============================================================================
// Repository Overview Generation
// ============================================================================

/// Generate a repository overview from analyzed summaries
pub fn generate_repo_overview(summaries: &[SemanticSummary], dir_path: &str) -> RepoOverview {
    let mut overview = RepoOverview::default();

    // Detect framework
    overview.framework = detect_framework(summaries);

    // Detect database
    overview.database = detect_database(summaries);

    // Detect package manager
    overview.package_manager = detect_package_manager(summaries);

    // Build module groups
    overview.modules = build_module_groups(summaries, dir_path);

    // Identify entry points
    overview.entry_points = identify_entry_points(summaries);

    // Build data flow
    overview.data_flow = build_data_flow(summaries);

    // Build stats
    overview.stats = build_stats(summaries);

    // Detect patterns
    overview.patterns = detect_patterns(summaries);

    overview
}

fn detect_framework(summaries: &[SemanticSummary]) -> Option<String> {
    for s in summaries {
        let file_lower = s.file.to_lowercase();

        // Next.js detection
        if file_lower.contains("next.config") || file_lower.contains("/app/layout") {
            return Some("Next.js (App Router)".to_string());
        }
        if file_lower.contains("/pages/") && (file_lower.ends_with(".tsx") || file_lower.ends_with(".jsx")) {
            return Some("Next.js (Pages Router)".to_string());
        }

        // React detection (if not Next.js)
        if s.insertions.iter().any(|i| i.contains("component")) {
            // Check for non-Next React
            if summaries.iter().all(|s| !s.file.to_lowercase().contains("next.config")) {
                return Some("React".to_string());
            }
        }

        // Express detection
        if s.added_dependencies.iter().any(|d| d == "express" || d == "Router") {
            return Some("Express.js".to_string());
        }
    }
    None
}

fn detect_database(summaries: &[SemanticSummary]) -> Option<String> {
    for s in summaries {
        // Drizzle detection
        if s.added_dependencies.iter().any(|d| d == "drizzle" || d == "pgTable" || d == "mysqlTable") {
            return Some("PostgreSQL (Drizzle ORM)".to_string());
        }

        // Prisma detection
        if s.file.to_lowercase().contains("prisma") {
            return Some("Prisma".to_string());
        }

        // Raw postgres
        if s.added_dependencies.iter().any(|d| d == "postgres" || d == "pg") {
            return Some("PostgreSQL".to_string());
        }
    }
    None
}

fn detect_package_manager(summaries: &[SemanticSummary]) -> Option<String> {
    for s in summaries {
        let file_lower = s.file.to_lowercase();

        if file_lower.ends_with("package-lock.json") {
            return Some("npm".to_string());
        }
        if file_lower.ends_with("pnpm-lock.yaml") {
            return Some("pnpm".to_string());
        }
        if file_lower.ends_with("yarn.lock") {
            return Some("yarn".to_string());
        }
        if file_lower.ends_with("cargo.toml") {
            return Some("cargo".to_string());
        }
    }
    None
}

fn build_module_groups(summaries: &[SemanticSummary], dir_path: &str) -> Vec<ModuleGroup> {
    let mut groups: HashMap<String, Vec<&SemanticSummary>> = HashMap::new();

    for s in summaries {
        // Get relative path
        let relative = s.file
            .strip_prefix(dir_path)
            .unwrap_or(&s.file)
            .trim_start_matches('/');

        // Determine module group
        let module = if relative.starts_with("src/app/api") || relative.contains("/api/") {
            "api".to_string()
        } else if relative.starts_with("src/db") || relative.contains("/db/") {
            "database".to_string()
        } else if relative.starts_with("src/app") || relative.starts_with("src/pages") {
            "pages".to_string()
        } else if relative.starts_with("src/components") {
            "components".to_string()
        } else if relative.starts_with("src/lib") || relative.starts_with("src/utils") {
            "lib".to_string()
        } else if !relative.contains('/') || relative.starts_with('.') {
            "config".to_string()
        } else {
            "other".to_string()
        };

        groups.entry(module).or_default().push(s);
    }

    groups
        .into_iter()
        .map(|(name, files)| {
            let purpose = match name.as_str() {
                "api" => "API route handlers".to_string(),
                "database" => "Database schema, migrations, seeds".to_string(),
                "pages" => "Page components and layouts".to_string(),
                "components" => "Reusable UI components".to_string(),
                "lib" => "Shared utilities and helpers".to_string(),
                "config" => "Configuration files".to_string(),
                _ => "Other files".to_string(),
            };

            // Calculate aggregate risk
            let high_count = files.iter().filter(|f| f.behavioral_risk == RiskLevel::High).count();
            let med_count = files.iter().filter(|f| f.behavioral_risk == RiskLevel::Medium).count();
            let risk = if high_count > 0 {
                RiskLevel::High
            } else if med_count > 0 {
                RiskLevel::Medium
            } else {
                RiskLevel::Low
            };

            // Get key files (high risk or with symbols)
            let key_files: Vec<String> = files
                .iter()
                .filter(|f| f.behavioral_risk == RiskLevel::High || f.symbol.is_some())
                .take(3)
                .map(|f| {
                    f.file
                        .rsplit('/')
                        .next()
                        .unwrap_or(&f.file)
                        .to_string()
                })
                .collect();

            ModuleGroup {
                name,
                purpose,
                file_count: files.len(),
                risk,
                key_files,
            }
        })
        .collect()
}

fn identify_entry_points(summaries: &[SemanticSummary]) -> Vec<String> {
    let mut entries = Vec::new();

    for s in summaries {
        let file_lower = s.file.to_lowercase();

        // Next.js entry points
        if file_lower.ends_with("page.tsx") || file_lower.ends_with("page.jsx") {
            entries.push(s.file.clone());
        }

        // API routes
        if file_lower.contains("/api/") && file_lower.ends_with("route.ts") {
            if let Some(ref sym) = s.symbol {
                let method = sym.to_uppercase();
                if matches!(method.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "PATCH") {
                    entries.push(format!("{} {}", method, s.file));
                }
            }
        }

        // Main/index files
        if file_lower.ends_with("main.rs")
            || file_lower.ends_with("index.ts")
            || file_lower.ends_with("index.js")
        {
            entries.push(s.file.clone());
        }
    }

    entries
}

fn build_data_flow(summaries: &[SemanticSummary]) -> HashMap<String, Vec<String>> {
    let mut flow = HashMap::new();

    for s in summaries {
        if !s.local_imports.is_empty() {
            flow.insert(s.file.clone(), s.local_imports.clone());
        }
    }

    flow
}

fn build_stats(summaries: &[SemanticSummary]) -> RepoStats {
    let mut stats = RepoStats::default();

    stats.total_files = summaries.len();

    for s in summaries {
        // Risk counts
        match s.behavioral_risk {
            RiskLevel::High => stats.high_risk += 1,
            RiskLevel::Medium => stats.medium_risk += 1,
            RiskLevel::Low => stats.low_risk += 1,
        }

        // Language counts
        *stats.by_language.entry(s.language.clone()).or_insert(0) += 1;

        // Component counts
        if s.symbol_kind == Some(SymbolKind::Component) {
            stats.components += 1;
        }

        // API endpoint counts
        if s.insertions.iter().any(|i| i.contains("API route")) {
            stats.api_endpoints += 1;
        }

        // Database table counts
        if s.insertions.iter().any(|i| i.contains("table definition")) {
            // Extract count from "database schema (N table definitions)"
            for insertion in &s.insertions {
                if insertion.contains("table definition") {
                    if let Some(count_str) = insertion.split('(').nth(1) {
                        if let Some(num) = count_str.split_whitespace().next() {
                            if let Ok(n) = num.parse::<usize>() {
                                stats.database_tables += n;
                            }
                        }
                    }
                }
            }
        }
    }

    stats
}

fn detect_patterns(summaries: &[SemanticSummary]) -> Vec<String> {
    let mut patterns = Vec::new();

    let has_api = summaries.iter().any(|s| s.insertions.iter().any(|i| i.contains("API route")));
    let has_db = summaries.iter().any(|s| s.insertions.iter().any(|i| i.contains("database")));
    let has_components = summaries.iter().any(|s| s.symbol_kind == Some(SymbolKind::Component));

    if has_api && has_db && has_components {
        patterns.push("Full-stack web application".to_string());
    } else if has_api && has_db {
        patterns.push("API with database backend".to_string());
    } else if has_components {
        patterns.push("React component library".to_string());
    }

    // Docker
    if summaries.iter().any(|s| s.file.to_lowercase().contains("docker")) {
        patterns.push("Dockerized deployment".to_string());
    }

    patterns
}

// ============================================================================
// Directory TOON Encoding (with overview)
// ============================================================================

/// Encode a full directory analysis as TOON (overview + files)
pub fn encode_toon_directory(overview: &RepoOverview, summaries: &[SemanticSummary]) -> String {
    let mut output = String::new();

    // Encode repository overview first
    output.push_str(&encode_repo_overview(overview));
    output.push_str("---\n");

    // Encode each file summary (filtered and cleaned)
    for summary in summaries {
        output.push_str(&encode_toon_clean(summary));
        output.push_str("---\n");
    }

    output
}

/// Encode repository overview as TOON
fn encode_repo_overview(overview: &RepoOverview) -> String {
    let mut obj = Map::new();

    obj.insert("_type".to_string(), json!("repo_overview"));

    if let Some(ref fw) = overview.framework {
        obj.insert("framework".to_string(), json!(fw));
    }

    if let Some(ref db) = overview.database {
        obj.insert("database".to_string(), json!(db));
    }

    if !overview.patterns.is_empty() {
        obj.insert("patterns".to_string(), json!(overview.patterns));
    }

    // Module summary
    if !overview.modules.is_empty() {
        let modules: Vec<Value> = overview
            .modules
            .iter()
            .map(|m| {
                json!({
                    "name": m.name,
                    "purpose": m.purpose,
                    "files": m.file_count,
                    "risk": m.risk.as_str()
                })
            })
            .collect();
        obj.insert("modules".to_string(), Value::Array(modules));
    }

    // Stats
    let stats = &overview.stats;
    obj.insert("files".to_string(), json!(stats.total_files));
    obj.insert(
        "risk_breakdown".to_string(),
        json!(format!(
            "high:{},medium:{},low:{}",
            stats.high_risk, stats.medium_risk, stats.low_risk
        )),
    );

    if stats.api_endpoints > 0 {
        obj.insert("api_endpoints".to_string(), json!(stats.api_endpoints));
    }
    if stats.database_tables > 0 {
        obj.insert("database_tables".to_string(), json!(stats.database_tables));
    }
    if stats.components > 0 {
        obj.insert("components".to_string(), json!(stats.components));
    }

    // Entry points
    if !overview.entry_points.is_empty() {
        obj.insert("entry_points".to_string(), json!(overview.entry_points));
    }

    let value = Value::Object(obj);
    encode_default(&value).unwrap_or_else(|e| format!("TOON encoding error: {}", e))
}

/// Encode a summary with filtered calls and no meaningless fields
pub fn encode_toon_clean(summary: &SemanticSummary) -> String {
    let mut obj = Map::new();

    // Simple scalar fields
    obj.insert("file".to_string(), json!(summary.file));
    obj.insert("language".to_string(), json!(summary.language));

    if let Some(ref sym) = summary.symbol {
        obj.insert("symbol".to_string(), json!(sym));
    }

    if let Some(ref kind) = summary.symbol_kind {
        obj.insert("symbol_kind".to_string(), json!(kind.as_str()));
    }

    if let Some(ref ret) = summary.return_type {
        obj.insert("return_type".to_string(), json!(ret));
    }

    // Skip public_surface_changed in directory mode (always false without diff)
    // Only include behavioral_risk if not low
    if summary.behavioral_risk != RiskLevel::Low {
        obj.insert(
            "behavioral_risk".to_string(),
            json!(risk_to_string(summary.behavioral_risk)),
        );
    }

    // Insertions array
    if !summary.insertions.is_empty() {
        obj.insert("insertions".to_string(), json!(summary.insertions));
    }

    // Added dependencies (only if non-empty)
    if !summary.added_dependencies.is_empty() {
        obj.insert(
            "added_dependencies".to_string(),
            json!(summary.added_dependencies),
        );
    }

    // Local imports for data flow (only if non-empty)
    if !summary.local_imports.is_empty() {
        obj.insert("imports_from".to_string(), json!(summary.local_imports));
    }

    // State changes
    if !summary.state_changes.is_empty() {
        let state_objs: Vec<Value> = summary
            .state_changes
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "type": s.state_type,
                    "init": s.initializer
                })
            })
            .collect();
        obj.insert("state".to_string(), Value::Array(state_objs));
    }

    // Control flow (only if present)
    if !summary.control_flow_changes.is_empty() {
        let kinds: Vec<&str> = summary
            .control_flow_changes
            .iter()
            .map(|c| c.kind.as_str())
            .collect();
        obj.insert("control_flow".to_string(), json!(kinds));
    }

    // Filtered calls (meaningful only)
    let meaningful_calls = filter_meaningful_calls(&summary.calls);
    if !meaningful_calls.is_empty() {
        let call_objs = build_deduplicated_calls(&meaningful_calls);
        obj.insert("calls".to_string(), Value::Array(call_objs));
    }

    // Raw fallback only if truly needed
    if let Some(ref raw) = summary.raw_fallback {
        if summary.insertions.is_empty()
            && summary.added_dependencies.is_empty()
            && summary.calls.is_empty()
            && summary.symbol.is_none()
        {
            // Handle empty files
            if raw.trim().is_empty() {
                obj.insert("note".to_string(), json!("(empty file)"));
            } else {
                let truncated: String = raw.lines().take(10).collect::<Vec<_>>().join("\n");
                let suffix = if raw.lines().count() > 10 {
                    "\n..."
                } else {
                    ""
                };
                obj.insert("raw".to_string(), json!(format!("{}{}", truncated, suffix)));
            }
        }
    }

    let value = Value::Object(obj);
    encode_default(&value).unwrap_or_else(|e| format!("TOON encoding error: {}", e))
}

/// Encode a semantic summary as TOON
pub fn encode_toon(summary: &SemanticSummary) -> String {
    // Build a JSON value that will encode nicely to TOON
    let mut obj = Map::new();

    // Simple scalar fields
    obj.insert("file".to_string(), json!(summary.file));
    obj.insert("language".to_string(), json!(summary.language));

    if let Some(ref sym) = summary.symbol {
        obj.insert("symbol".to_string(), json!(sym));
    }

    if let Some(ref kind) = summary.symbol_kind {
        obj.insert("symbol_kind".to_string(), json!(kind.as_str()));
    }

    if let Some(ref ret) = summary.return_type {
        obj.insert("return_type".to_string(), json!(ret));
    }

    obj.insert(
        "public_surface_changed".to_string(),
        json!(summary.public_surface_changed),
    );
    obj.insert(
        "behavioral_risk".to_string(),
        json!(risk_to_string(summary.behavioral_risk)),
    );

    // Insertions array
    if !summary.insertions.is_empty() {
        obj.insert("insertions".to_string(), json!(summary.insertions));
    }

    // Added dependencies
    if !summary.added_dependencies.is_empty() {
        obj.insert(
            "added_dependencies".to_string(),
            json!(summary.added_dependencies),
        );
    }

    // State changes - convert to uniform array of objects for tabular format
    if !summary.state_changes.is_empty() {
        let state_objs: Vec<Value> = summary
            .state_changes
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "type": s.state_type,
                    "initializer": s.initializer
                })
            })
            .collect();
        obj.insert("state_changes".to_string(), Value::Array(state_objs));
    }

    // Arguments - convert to uniform array of objects for tabular format
    if !summary.arguments.is_empty() {
        let arg_objs: Vec<Value> = summary
            .arguments
            .iter()
            .map(|a| {
                json!({
                    "name": a.name,
                    "type": a.arg_type.as_deref().unwrap_or("_"),
                    "default": a.default_value.as_deref().unwrap_or("_")
                })
            })
            .collect();
        obj.insert("arguments".to_string(), Value::Array(arg_objs));
    }

    // Props - convert to uniform array of objects for tabular format
    if !summary.props.is_empty() {
        let prop_objs: Vec<Value> = summary
            .props
            .iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "type": p.prop_type.as_deref().unwrap_or("_"),
                    "default": p.default_value.as_deref().unwrap_or("_"),
                    "required": p.required
                })
            })
            .collect();
        obj.insert("props".to_string(), Value::Array(prop_objs));
    }

    // Control flow changes - just extract the kinds
    if !summary.control_flow_changes.is_empty() {
        let kinds: Vec<&str> = summary
            .control_flow_changes
            .iter()
            .map(|c| c.kind.as_str())
            .collect();
        obj.insert("control_flow".to_string(), json!(kinds));
    }

    // Function calls with context (deduplicated, counted)
    if !summary.calls.is_empty() {
        let call_objs = build_deduplicated_calls(&summary.calls);
        obj.insert("calls".to_string(), Value::Array(call_objs));
    }

    // Raw fallback - only include if we have no semantic data at all
    if let Some(ref raw) = summary.raw_fallback {
        if summary.added_dependencies.is_empty()
            && summary.calls.is_empty()
            && summary.state_changes.is_empty()
            && summary.control_flow_changes.is_empty()
            && summary.symbol.is_none()
        {
            // Truncate to 20 lines
            let truncated: String = raw
                .lines()
                .take(20)
                .collect::<Vec<_>>()
                .join("\n");
            let suffix = if raw.lines().count() > 20 {
                "\n...(truncated)"
            } else {
                ""
            };
            obj.insert(
                "raw_source".to_string(),
                json!(format!("{}{}", truncated, suffix)),
            );
        }
    }

    // Encode to TOON using rtoon
    let value = Value::Object(obj);
    encode_default(&value).unwrap_or_else(|e| format!("TOON encoding error: {}", e))
}

/// Build deduplicated and counted call objects
fn build_deduplicated_calls(calls: &[crate::schema::Call]) -> Vec<Value> {
    // Deduplicate calls by (name, object, awaited, in_try) and count occurrences
    let mut call_counts: HashMap<(String, String, bool, bool), usize> = HashMap::new();

    for call in calls {
        let key = (
            call.name.clone(),
            call.object.clone().unwrap_or_default(),
            call.is_awaited,
            call.in_try,
        );
        *call_counts.entry(key).or_insert(0) += 1;
    }

    // Convert to sorted vec for deterministic output
    let mut unique_calls: Vec<_> = call_counts.into_iter().collect();
    unique_calls.sort_by(|a, b| b.1.cmp(&a.1).then(a.0 .0.cmp(&b.0 .0))); // Sort by count desc, then name

    unique_calls
        .into_iter()
        .map(|((name, obj, awaited, in_try), count)| {
            json!({
                "name": name,
                "obj": if obj.is_empty() { "_".to_string() } else { obj },
                "await": if awaited { "Y" } else { "_" },
                "try": if in_try { "Y" } else { "_" },
                "count": if count > 1 { count.to_string() } else { "_".to_string() }
            })
        })
        .collect()
}

/// Convert risk level to string
fn risk_to_string(risk: RiskLevel) -> &'static str {
    risk.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ControlFlowChange, ControlFlowKind, Location, StateChange, SymbolKind};

    #[test]
    fn test_basic_toon_output() {
        let summary = SemanticSummary {
            file: "test.tsx".to_string(),
            language: "tsx".to_string(),
            symbol: Some("AppLayout".to_string()),
            symbol_kind: Some(SymbolKind::Component),
            return_type: Some("JSX.Element".to_string()),
            public_surface_changed: false,
            behavioral_risk: RiskLevel::Medium,
            ..Default::default()
        };

        let toon = encode_toon(&summary);

        assert!(toon.contains("file:"));
        assert!(toon.contains("test.tsx"));
        assert!(toon.contains("language:"));
        assert!(toon.contains("tsx"));
        assert!(toon.contains("symbol:"));
        assert!(toon.contains("AppLayout"));
        assert!(toon.contains("symbol_kind:"));
        assert!(toon.contains("component"));
        assert!(toon.contains("return_type:"));
        assert!(toon.contains("JSX.Element"));
        assert!(toon.contains("public_surface_changed:"));
        assert!(toon.contains("false"));
        assert!(toon.contains("behavioral_risk:"));
        assert!(toon.contains("medium"));
    }

    #[test]
    fn test_insertions_format() {
        let summary = SemanticSummary {
            file: "test.tsx".to_string(),
            language: "tsx".to_string(),
            insertions: vec![
                "header container with nav".to_string(),
                "6 route links".to_string(),
            ],
            ..Default::default()
        };

        let toon = encode_toon(&summary);

        assert!(toon.contains("insertions"));
        assert!(toon.contains("header container with nav"));
        assert!(toon.contains("6 route links"));
    }

    #[test]
    fn test_state_changes_tabular() {
        let summary = SemanticSummary {
            file: "test.tsx".to_string(),
            language: "tsx".to_string(),
            state_changes: vec![StateChange {
                name: "open".to_string(),
                state_type: "boolean".to_string(),
                initializer: "false".to_string(),
            }],
            ..Default::default()
        };

        let toon = encode_toon(&summary);

        assert!(toon.contains("state_changes"));
        assert!(toon.contains("open"));
        assert!(toon.contains("boolean"));
        assert!(toon.contains("false"));
    }

    #[test]
    fn test_dependencies_inline() {
        let summary = SemanticSummary {
            file: "test.tsx".to_string(),
            language: "tsx".to_string(),
            added_dependencies: vec!["useState".to_string(), "Link".to_string()],
            ..Default::default()
        };

        let toon = encode_toon(&summary);

        assert!(toon.contains("added_dependencies"));
        assert!(toon.contains("useState"));
        assert!(toon.contains("Link"));
    }

    #[test]
    fn test_control_flow_inline() {
        let summary = SemanticSummary {
            file: "test.tsx".to_string(),
            language: "tsx".to_string(),
            control_flow_changes: vec![
                ControlFlowChange {
                    kind: ControlFlowKind::If,
                    location: Location::default(),
                },
                ControlFlowChange {
                    kind: ControlFlowKind::For,
                    location: Location::default(),
                },
            ],
            ..Default::default()
        };

        let toon = encode_toon(&summary);

        assert!(toon.contains("control_flow"));
        assert!(toon.contains("if"));
        assert!(toon.contains("for"));
    }

    #[test]
    fn test_raw_fallback() {
        let summary = SemanticSummary {
            file: "test.tsx".to_string(),
            language: "tsx".to_string(),
            raw_fallback: Some("function foo() {}".to_string()),
            ..Default::default()
        };

        let toon = encode_toon(&summary);

        assert!(toon.contains("raw_source"));
        assert!(toon.contains("function foo() {}"));
    }
}
