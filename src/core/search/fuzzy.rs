#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub index: usize,
    pub score: i32,
    pub matched_indices: Vec<usize>,
    pub ranges: Vec<(usize, usize)>,
}

pub fn match_candidates(query: &str, candidates: &[String]) -> Vec<FuzzyMatch> {
    let query = query.trim();
    let mut matches = Vec::new();

    for (index, candidate) in candidates.iter().enumerate() {
        if query.is_empty() {
            matches.push(FuzzyMatch {
                index,
                score: 0,
                matched_indices: Vec::new(),
                ranges: Vec::new(),
            });
            continue;
        }

        if let Some(matched_indices) = match_indices(query, candidate) {
            let score = score_match(candidate, &matched_indices, query);
            let ranges = indices_to_ranges(&matched_indices);
            matches.push(FuzzyMatch {
                index,
                score,
                matched_indices,
                ranges,
            });
        }
    }

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
}

pub fn match_candidates_limited(
    query: &str,
    candidates: &[String],
    limit: usize,
) -> Vec<FuzzyMatch> {
    if limit == 0 {
        return Vec::new();
    }
    let mut matches = match_candidates(query, candidates);
    if matches.len() > limit {
        matches.truncate(limit);
    }
    matches
}

pub fn match_candidates_top(
    query: &str,
    candidates: &[String],
    limit: usize,
) -> Vec<FuzzyMatch> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    if limit == 0 {
        return Vec::new();
    }

    let query = query.trim();
    let mut heap: BinaryHeap<Reverse<(i32, usize)>> = BinaryHeap::new();
    let mut matches = Vec::new();

    for (index, candidate) in candidates.iter().enumerate() {
        let matched_indices = if query.is_empty() {
            Vec::new()
        } else if let Some(indices) = match_indices(query, candidate) {
            indices
        } else {
            continue;
        };

        let score = score_match(candidate, &matched_indices, query);
        let ranges = indices_to_ranges(&matched_indices);
        let fm = FuzzyMatch {
            index,
            score,
            matched_indices,
            ranges,
        };
        matches.push(fm);
        let fm_index = matches.len() - 1;

        if heap.len() < limit {
            heap.push(Reverse((score, fm_index)));
        } else if let Some(Reverse((min_score, min_idx))) = heap.peek() {
            if score > *min_score || (score == *min_score && index < matches[*min_idx].index) {
                heap.pop();
                heap.push(Reverse((score, fm_index)));
            }
        }
    }

    let mut out = heap
        .into_iter()
        .map(|r| matches[r.0 .1].clone())
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.score.cmp(&a.score));
    out
}

fn match_indices(query: &str, candidate: &str) -> Option<Vec<usize>> {
    let query_chars: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    let candidate_chars: Vec<char> = candidate.chars().map(|c| c.to_ascii_lowercase()).collect();

    if query_chars.is_empty() {
        return Some(Vec::new());
    }

    if query_chars.len() <= candidate_chars.len() {
        for start in 0..=candidate_chars.len() - query_chars.len() {
            if candidate_chars[start..start + query_chars.len()] == query_chars[..] {
                return Some((start..start + query_chars.len()).collect());
            }
        }
    }

    let mut indices = Vec::new();
    let mut qpos = 0;

    for (cpos, ch) in candidate_chars.iter().enumerate() {
        if qpos >= query_chars.len() {
            break;
        }
        if *ch == query_chars[qpos] {
            indices.push(cpos);
            qpos += 1;
        }
    }

    if qpos == query_chars.len() {
        Some(indices)
    } else {
        None
    }
}

fn score_match(candidate: &str, matched_indices: &[usize], query: &str) -> i32 {
    if matched_indices.is_empty() {
        return 0;
    }

    let chars: Vec<char> = candidate.chars().collect();
    let mut score = (matched_indices.len() as i32) * 10;
    let query = query.trim();
    let query_len = query.chars().count();
    let candidate_lower = candidate.to_ascii_lowercase();
    let query_lower = query.to_ascii_lowercase();

    if matched_indices.len() == query_len {
        score += 20;
    }

    if let Some(first) = matched_indices.first() {
        if *first == 0 {
            score += 30;
        }
    }

    let mut consecutive_run = 1;
    for pair in matched_indices.windows(2) {
        if pair[1] == pair[0] + 1 {
            consecutive_run += 1;
            score += 8;
        } else {
            let gap = pair[1].saturating_sub(pair[0] + 1) as i32;
            score -= gap * 2;
        }
    }

    if consecutive_run == matched_indices.len() {
        score += 40;
    }

    for &idx in matched_indices {
        if idx == 0 {
            score += 15;
            continue;
        }
        if is_boundary(chars[idx - 1]) {
            score += 12;
        }
        if chars[idx].is_uppercase() && chars[idx - 1].is_lowercase() {
            score += 10;
        }
    }

    if !query_lower.is_empty() {
        if candidate_lower.ends_with(&query_lower) {
            score += 80;
        }
        if query_lower.starts_with('.') && candidate_lower.ends_with(&query_lower) {
            score += 40;
        }
        if contains_whole_segment(&candidate_lower, &query_lower) {
            score += 60;
        }
    }

    let len_penalty = (chars.len() as i32) / 2;
    score -= len_penalty;

    score
}

fn indices_to_ranges(indices: &[usize]) -> Vec<(usize, usize)> {
    if indices.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = indices[0];
    let mut prev = indices[0];

    for &idx in indices.iter().skip(1) {
        if idx == prev + 1 {
            prev = idx;
            continue;
        }
        ranges.push((start, prev + 1));
        start = idx;
        prev = idx;
    }

    ranges.push((start, prev + 1));
    ranges
}

fn is_boundary(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '/' | '\\' | '_' | '-' | '.' | ':')
}

fn contains_whole_segment(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    for (idx, _) in hay.match_indices(needle) {
        let before = if idx == 0 {
            None
        } else {
            hay[..idx].chars().last()
        };
        let after_idx = idx + needle.len();
        let after = if after_idx >= hay.len() {
            None
        } else {
            hay[after_idx..].chars().next()
        };
        let before_ok = before.map(is_boundary).unwrap_or(true);
        let after_ok = after.map(is_boundary).unwrap_or(true);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}
