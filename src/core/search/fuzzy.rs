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
            let score = score_match(candidate, &matched_indices, query.chars().count());
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

fn score_match(candidate: &str, matched_indices: &[usize], query_len: usize) -> i32 {
    if matched_indices.is_empty() {
        return 0;
    }

    let chars: Vec<char> = candidate.chars().collect();
    let mut score = (matched_indices.len() as i32) * 10;

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
