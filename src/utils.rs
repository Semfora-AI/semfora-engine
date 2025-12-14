//! Utility functions for safe string operations
//!
//! This module provides UTF-8 safe string manipulation utilities to prevent
//! panics when working with multi-byte Unicode characters.

/// Safely truncate a string at a UTF-8 char boundary.
///
/// When truncating a string to a specific byte length, this function ensures
/// the truncation point is at a valid UTF-8 character boundary, preventing
/// panics that would occur from slicing in the middle of a multi-byte character.
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_bytes` - Maximum byte length for the result
///
/// # Returns
/// A string slice that is at most `max_bytes` long, truncated at a valid
/// character boundary.
///
/// # Examples
/// ```
/// use semfora_engine::utils::truncate_to_char_boundary;
///
/// // ASCII string
/// assert_eq!(truncate_to_char_boundary("hello world", 5), "hello");
///
/// // Multi-byte UTF-8 characters (e.g., Punjabi digit ੨ is 3 bytes)
/// let s = "abc੨def"; // '੨' spans bytes 3-5
/// assert_eq!(truncate_to_char_boundary(s, 4), "abc"); // Truncates before ੨
/// assert_eq!(truncate_to_char_boundary(s, 6), "abc੨"); // Includes full ੨
/// ```
pub fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the last valid char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Safely truncate a string and append an ellipsis.
///
/// This is a convenience wrapper around `truncate_to_char_boundary` that
/// appends "..." to indicate truncation occurred.
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_bytes` - Maximum byte length before the ellipsis
///
/// # Returns
/// The original string if it's short enough, otherwise a truncated string
/// with "..." appended.
pub fn truncate_with_ellipsis(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        format!("{}...", truncate_to_char_boundary(s, max_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_ascii_short() {
        assert_eq!(truncate_to_char_boundary("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_ascii_exact() {
        assert_eq!(truncate_to_char_boundary("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_ascii_cut() {
        assert_eq!(truncate_to_char_boundary("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_utf8_2byte() {
        // 'é' is a 2-byte UTF-8 character (0xC3 0xA9)
        let s = "héllo";
        // h=0, é=1-2, l=3, l=4, o=5
        assert_eq!(truncate_to_char_boundary(s, 3), "hé"); // Includes full é
        assert_eq!(truncate_to_char_boundary(s, 2), "h"); // Can't fit é, just h
    }

    #[test]
    fn test_truncate_utf8_3byte() {
        // '੨' (Punjabi digit two) is a 3-byte UTF-8 character (0xE0 0xA9 0xA8)
        let s = "abc੨def";
        // a=0, b=1, c=2, ੨=3-5, d=6, e=7, f=8
        assert_eq!(truncate_to_char_boundary(s, 3), "abc"); // Before ੨
        assert_eq!(truncate_to_char_boundary(s, 4), "abc"); // Inside ੨, truncate before
        assert_eq!(truncate_to_char_boundary(s, 5), "abc"); // Inside ੨, truncate before
        assert_eq!(truncate_to_char_boundary(s, 6), "abc੨"); // After ੨
    }

    #[test]
    fn test_truncate_utf8_4byte() {
        // '𐍈' is a 4-byte UTF-8 character (Gothic letter hwair)
        let s = "ab𐍈cd";
        // a=0, b=1, 𐍈=2-5, c=6, d=7
        assert_eq!(truncate_to_char_boundary(s, 2), "ab"); // Before 𐍈
        assert_eq!(truncate_to_char_boundary(s, 3), "ab"); // Inside 𐍈
        assert_eq!(truncate_to_char_boundary(s, 5), "ab"); // Inside 𐍈
        assert_eq!(truncate_to_char_boundary(s, 6), "ab𐍈"); // After 𐍈
    }

    #[test]
    fn test_truncate_empty_string() {
        assert_eq!(truncate_to_char_boundary("", 10), "");
        assert_eq!(truncate_to_char_boundary("", 0), "");
    }

    #[test]
    fn test_truncate_zero_bytes() {
        assert_eq!(truncate_to_char_boundary("hello", 0), "");
        assert_eq!(truncate_to_char_boundary("੨", 0), "");
    }

    #[test]
    fn test_truncate_with_ellipsis_short() {
        assert_eq!(truncate_with_ellipsis("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_with_ellipsis_long() {
        assert_eq!(truncate_with_ellipsis("hello world", 5), "hello...");
    }

    #[test]
    fn test_truncate_with_ellipsis_utf8() {
        let s = "abc੨def";
        assert_eq!(truncate_with_ellipsis(s, 4), "abc...");
    }

    #[test]
    fn test_real_world_icu_string() {
        // This is the actual string that caused the panic
        let s = r#"icu::UnicodeSet( icu::UnicodeString::fromUTF8("[θ२২੨੨૨೩೭շзҙӡउওਤ੩૩౩ဒვპੜკ੫丩ㄐճ৪੪୫૭୨౨]"), status)"#;

        // The character '੨' starts at byte 56 and ends at byte 59
        // Truncating at 57 should safely go back to 56 (before the character)
        let result = truncate_to_char_boundary(s, 57);
        assert!(result.len() <= 57);
        assert!(result.is_char_boundary(result.len())); // Should be valid UTF-8

        // Verify we can safely format it
        let formatted = truncate_with_ellipsis(s, 57);
        assert!(formatted.ends_with("..."));
    }
}
