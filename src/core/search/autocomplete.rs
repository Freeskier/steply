use crate::core::search::fuzzy::FuzzyMatch;

pub fn suggest(query: &str, matches: &[FuzzyMatch], candidates: &[String]) -> Option<String> {
    if query.trim().is_empty() {
        return None;
    }

    let best = matches.first()?;
    let candidate = candidates.get(best.index)?;

    if candidate
        .to_ascii_lowercase()
        .starts_with(&query.to_ascii_lowercase())
    {
        Some(candidate.clone())
    } else {
        None
    }
}
