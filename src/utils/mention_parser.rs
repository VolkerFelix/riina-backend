use regex::Regex;
use std::sync::OnceLock;

/// Regex pattern for matching @username mentions
/// Matches @ followed by alphanumeric characters and underscores
static MENTION_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_mention_regex() -> &'static Regex {
    MENTION_REGEX.get_or_init(|| {
        Regex::new(r"@([a-zA-Z0-9_]+)").expect("Failed to compile mention regex")
    })
}

/// Extract all @username mentions from text
///
/// # Arguments
/// * `text` - The text to search for mentions
///
/// # Returns
/// A Vec of usernames (without the @ symbol) found in the text.
/// Duplicates are preserved to maintain mention context.
///
/// # Examples
/// ```
/// use riina_backend::utils::mention_parser::extract_mentions;
/// let mentions = extract_mentions("Hey @john and @jane, check this out!");
/// assert_eq!(mentions, vec!["john", "jane"]);
/// ```
pub fn extract_mentions(text: &str) -> Vec<String> {
    let regex = get_mention_regex();

    regex
        .captures_iter(text)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// Extract unique @username mentions from text (removes duplicates)
///
/// # Arguments
/// * `text` - The text to search for mentions
///
/// # Returns
/// A Vec of unique usernames (without the @ symbol) found in the text.
///
/// # Examples
/// ```
/// use riina_backend::utils::mention_parser::extract_unique_mentions;
/// let mentions = extract_unique_mentions("Hey @john and @john again!");
/// assert_eq!(mentions, vec!["john"]);
/// ```
pub fn extract_unique_mentions(text: &str) -> Vec<String> {
    let mentions = extract_mentions(text);
    let mut unique_mentions = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for mention in mentions {
        if seen.insert(mention.clone()) {
            unique_mentions.push(mention);
        }
    }

    unique_mentions
}
