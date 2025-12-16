// INTENTIONALLY BAD CODE - FOR TESTING SEMFORA-CI SEMANTIC ANALYSIS
// This file contains anti-patterns that should be flagged by semantic analysis

use std::collections::HashMap;

/// A god function that does way too many things - extremely high complexity
pub fn do_everything_badly(
    input: &str,
    count: i32,
    flag1: bool,
    flag2: bool,
    flag3: bool,
    flag4: bool,
    mode: &str,
    options: Option<HashMap<String, String>>,
) -> Result<String, String> {
    let mut result = String::new();
    let mut counter = 0;
    let mut temp_storage: Vec<String> = Vec::new();
    let mut another_counter = 0;
    let mut yet_another = 0;

    // Deeply nested conditionals - high cyclomatic complexity
    if flag1 {
        if flag2 {
            if flag3 {
                if flag4 {
                    if mode == "a" {
                        if count > 0 {
                            if count < 100 {
                                if input.len() > 5 {
                                    if input.starts_with("x") {
                                        if input.ends_with("y") {
                                            result.push_str("deep nesting 1");
                                            counter += 1;
                                        } else {
                                            result.push_str("deep nesting 2");
                                            counter += 2;
                                        }
                                    } else {
                                        result.push_str("deep nesting 3");
                                        counter += 3;
                                    }
                                } else {
                                    result.push_str("deep nesting 4");
                                    counter += 4;
                                }
                            } else {
                                result.push_str("deep nesting 5");
                                counter += 5;
                            }
                        } else {
                            result.push_str("deep nesting 6");
                            counter += 6;
                        }
                    } else if mode == "b" {
                        for i in 0..count {
                            for j in 0..count {
                                for k in 0..10 {
                                    if i % 2 == 0 {
                                        if j % 3 == 0 {
                                            if k % 5 == 0 {
                                                temp_storage.push(format!("{}-{}-{}", i, j, k));
                                                another_counter += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if mode == "c" {
                        match input {
                            "one" => {
                                if flag1 { result.push_str("1a"); }
                                if flag2 { result.push_str("1b"); }
                                if flag3 { result.push_str("1c"); }
                            }
                            "two" => {
                                if flag1 { result.push_str("2a"); }
                                if flag2 { result.push_str("2b"); }
                                if flag3 { result.push_str("2c"); }
                            }
                            "three" => {
                                if flag1 { result.push_str("3a"); }
                                if flag2 { result.push_str("3b"); }
                                if flag3 { result.push_str("3c"); }
                            }
                            _ => {
                                if flag1 { result.push_str("xa"); }
                                if flag2 { result.push_str("xb"); }
                                if flag3 { result.push_str("xc"); }
                            }
                        }
                    }
                }
            }
        }
    }

    // More complexity with options handling
    if let Some(opts) = options {
        for (key, value) in opts.iter() {
            if key == "process" {
                if value == "yes" {
                    for c in input.chars() {
                        if c.is_alphabetic() {
                            if c.is_uppercase() {
                                result.push(c.to_ascii_lowercase());
                            } else {
                                result.push(c.to_ascii_uppercase());
                            }
                        } else if c.is_numeric() {
                            result.push(c);
                            yet_another += 1;
                        } else {
                            result.push('_');
                        }
                    }
                }
            } else if key == "validate" {
                if value == "strict" {
                    if input.len() < 3 {
                        return Err("Too short".to_string());
                    }
                    if input.len() > 1000 {
                        return Err("Too long".to_string());
                    }
                    if !input.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        return Err("Invalid chars".to_string());
                    }
                }
            }
        }
    }

    // Even more branches
    match count {
        0 => result.push_str("zero"),
        1 => result.push_str("one"),
        2 => result.push_str("two"),
        3 => result.push_str("three"),
        4 => result.push_str("four"),
        5 => result.push_str("five"),
        6 => result.push_str("six"),
        7 => result.push_str("seven"),
        8 => result.push_str("eight"),
        9 => result.push_str("nine"),
        10..=20 => result.push_str("teens"),
        21..=50 => result.push_str("twenties-to-fifties"),
        51..=100 => result.push_str("fifties-to-hundred"),
        _ => result.push_str("large"),
    }

    result.push_str(&format!(" counters: {}, {}, {}", counter, another_counter, yet_another));
    result.push_str(&format!(" temp_len: {}", temp_storage.len()));

    Ok(result)
}

// DUPLICATE FUNCTION 1 - Copy-pasted code
pub fn process_data_variant_a(items: &[String], threshold: usize) -> Vec<String> {
    let mut results = Vec::new();
    for item in items {
        if item.len() > threshold {
            let processed = item.to_uppercase();
            let trimmed = processed.trim().to_string();
            if !trimmed.is_empty() {
                results.push(trimmed);
            }
        }
    }
    results.sort();
    results.dedup();
    results
}

// DUPLICATE FUNCTION 2 - Nearly identical to above
pub fn process_data_variant_b(items: &[String], threshold: usize) -> Vec<String> {
    let mut results = Vec::new();
    for item in items {
        if item.len() > threshold {
            let processed = item.to_uppercase();
            let trimmed = processed.trim().to_string();
            if !trimmed.is_empty() {
                results.push(trimmed);
            }
        }
    }
    results.sort();
    results.dedup();
    results
}

// DUPLICATE FUNCTION 3 - Another copy
pub fn process_data_variant_c(items: &[String], threshold: usize) -> Vec<String> {
    let mut results = Vec::new();
    for item in items {
        if item.len() > threshold {
            let processed = item.to_uppercase();
            let trimmed = processed.trim().to_string();
            if !trimmed.is_empty() {
                results.push(trimmed);
            }
        }
    }
    results.sort();
    results.dedup();
    results
}

// DUPLICATE FUNCTION 4 - Yet another copy
pub fn process_data_variant_d(items: &[String], threshold: usize) -> Vec<String> {
    let mut results = Vec::new();
    for item in items {
        if item.len() > threshold {
            let processed = item.to_uppercase();
            let trimmed = processed.trim().to_string();
            if !trimmed.is_empty() {
                results.push(trimmed);
            }
        }
    }
    results.sort();
    results.dedup();
    results
}

/// Another complex function with many branches
pub fn complex_calculator(
    op: &str,
    a: f64,
    b: f64,
    precision: Option<u32>,
    allow_negative: bool,
    allow_zero: bool,
    clamp_result: bool,
    min_val: f64,
    max_val: f64,
) -> Result<f64, String> {
    let result = match op {
        "add" => a + b,
        "sub" => a - b,
        "mul" => a * b,
        "div" => {
            if b == 0.0 {
                if allow_zero {
                    0.0
                } else {
                    return Err("Division by zero".to_string());
                }
            } else {
                a / b
            }
        }
        "pow" => {
            if a < 0.0 && b.fract() != 0.0 {
                if allow_negative {
                    -((-a).powf(b))
                } else {
                    return Err("Negative base with fractional exponent".to_string());
                }
            } else {
                a.powf(b)
            }
        }
        "mod" => {
            if b == 0.0 {
                return Err("Modulo by zero".to_string());
            }
            a % b
        }
        "max" => a.max(b),
        "min" => a.min(b),
        "avg" => (a + b) / 2.0,
        _ => return Err(format!("Unknown operation: {}", op)),
    };

    let result = if let Some(p) = precision {
        let factor = 10_f64.powi(p as i32);
        (result * factor).round() / factor
    } else {
        result
    };

    let result = if !allow_negative && result < 0.0 {
        if clamp_result {
            0.0
        } else {
            return Err("Negative result not allowed".to_string());
        }
    } else {
        result
    };

    let result = if clamp_result {
        result.clamp(min_val, max_val)
    } else {
        if result < min_val {
            return Err(format!("Result {} below minimum {}", result, min_val));
        }
        if result > max_val {
            return Err(format!("Result {} above maximum {}", result, max_val));
        }
        result
    };

    Ok(result)
}

/// More duplicates - validation functions
pub fn validate_string_format_1(s: &str) -> bool {
    if s.is_empty() { return false; }
    if s.len() > 255 { return false; }
    let first = s.chars().next().unwrap();
    if !first.is_alphabetic() { return false; }
    for c in s.chars() {
        if !c.is_alphanumeric() && c != '_' && c != '-' {
            return false;
        }
    }
    true
}

pub fn validate_string_format_2(s: &str) -> bool {
    if s.is_empty() { return false; }
    if s.len() > 255 { return false; }
    let first = s.chars().next().unwrap();
    if !first.is_alphabetic() { return false; }
    for c in s.chars() {
        if !c.is_alphanumeric() && c != '_' && c != '-' {
            return false;
        }
    }
    true
}

pub fn validate_string_format_3(s: &str) -> bool {
    if s.is_empty() { return false; }
    if s.len() > 255 { return false; }
    let first = s.chars().next().unwrap();
    if !first.is_alphabetic() { return false; }
    for c in s.chars() {
        if !c.is_alphanumeric() && c != '_' && c != '-' {
            return false;
        }
    }
    true
}

/// Massive switch/match statement
pub fn handle_command(cmd: &str, args: &[&str]) -> String {
    match cmd {
        "help" => "Available commands: help, version, status, start, stop, restart, reload, config, logs, stats, info, debug, trace, test, benchmark, profile, analyze, report, export, import, backup, restore, migrate, upgrade, downgrade, install, uninstall, enable, disable, list, show, get, set, add, remove, update, delete, create, destroy, init, reset, clear, flush, sync, async, queue, dequeue, push, pop, peek, size, count, len, empty, full, contains, find, search, filter, map, reduce, fold, collect, iter, next, prev, first, last, nth, take, skip, reverse, sort, shuffle, unique, group, split, join, merge, diff, patch, clone, copy, move, rename, link, unlink".to_string(),
        "version" => "1.0.0".to_string(),
        "status" => {
            if args.is_empty() {
                "Status: OK".to_string()
            } else if args[0] == "verbose" {
                "Status: OK\nUptime: 1234s\nMemory: 56MB\nCPU: 12%".to_string()
            } else if args[0] == "json" {
                r#"{"status":"ok","uptime":1234,"memory":56,"cpu":12}"#.to_string()
            } else {
                "Status: OK".to_string()
            }
        }
        "start" => "Started".to_string(),
        "stop" => "Stopped".to_string(),
        "restart" => "Restarted".to_string(),
        "reload" => "Reloaded".to_string(),
        "config" => {
            if args.is_empty() {
                "Config: default".to_string()
            } else {
                format!("Config: {}", args.join(" "))
            }
        }
        "logs" => "No logs available".to_string(),
        "stats" => "Stats: 0 events".to_string(),
        "info" => "Info: semfora-engine".to_string(),
        "debug" => "Debug mode enabled".to_string(),
        "trace" => "Trace mode enabled".to_string(),
        "test" => "All tests passed".to_string(),
        "benchmark" => "Benchmark complete".to_string(),
        "profile" => "Profile data collected".to_string(),
        "analyze" => "Analysis complete".to_string(),
        "report" => "Report generated".to_string(),
        "export" => "Data exported".to_string(),
        "import" => "Data imported".to_string(),
        "backup" => "Backup created".to_string(),
        "restore" => "Backup restored".to_string(),
        "migrate" => "Migration complete".to_string(),
        "upgrade" => "Upgrade complete".to_string(),
        "downgrade" => "Downgrade complete".to_string(),
        "install" => "Installation complete".to_string(),
        "uninstall" => "Uninstallation complete".to_string(),
        "enable" => "Feature enabled".to_string(),
        "disable" => "Feature disabled".to_string(),
        "list" => "Listing items...".to_string(),
        "show" => "Showing details...".to_string(),
        "get" => "Getting value...".to_string(),
        "set" => "Setting value...".to_string(),
        "add" => "Adding item...".to_string(),
        "remove" => "Removing item...".to_string(),
        "update" => "Updating item...".to_string(),
        "delete" => "Deleting item...".to_string(),
        "create" => "Creating resource...".to_string(),
        "destroy" => "Destroying resource...".to_string(),
        "init" => "Initializing...".to_string(),
        "reset" => "Resetting...".to_string(),
        "clear" => "Clearing...".to_string(),
        "flush" => "Flushing...".to_string(),
        "sync" => "Synchronizing...".to_string(),
        "queue" => "Queuing...".to_string(),
        "push" => "Pushing...".to_string(),
        "pop" => "Popping...".to_string(),
        _ => format!("Unknown command: {}", cmd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_code_compiles() {
        // Just verify it compiles
        let _ = do_everything_badly("test", 5, true, true, true, true, "a", None);
    }
}
