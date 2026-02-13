#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub index: usize,
    pub score: i32,
    pub ranges: Vec<(usize, usize)>,
}

pub fn ranked_matches(query: &str, candidates: &[String]) -> Vec<FuzzyMatch> {
    let query = query.trim();
    let mut out = Vec::<FuzzyMatch>::new();

    for (index, candidate) in candidates.iter().enumerate() {
        if let Some(indices) = match_indices(query, candidate.as_str()) {
            let score = score_match(candidate.as_str(), indices.as_slice(), query);
            out.push(FuzzyMatch {
                index,
                score,
                ranges: indices_to_ranges(indices.as_slice()),
            });
        }
    }

    out.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.index.cmp(&right.index))
    });
    out
}

fn match_indices(query: &str, candidate: &str) -> Option<Vec<usize>> {
    let query_chars: Vec<char> = query.chars().map(|ch| ch.to_ascii_lowercase()).collect();
    let candidate_chars: Vec<char> = candidate
        .chars()
        .map(|ch| ch.to_ascii_lowercase())
        .collect();

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

    let mut indices = Vec::<usize>::new();
    let mut q_pos = 0usize;
    for (c_pos, ch) in candidate_chars.iter().enumerate() {
        if q_pos >= query_chars.len() {
            break;
        }
        if *ch == query_chars[q_pos] {
            indices.push(c_pos);
            q_pos += 1;
        }
    }

    if q_pos == query_chars.len() {
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
    let query_len = query.chars().count();
    let candidate_lower = candidate.to_ascii_lowercase();
    let query_lower = query.to_ascii_lowercase();

    if matched_indices.len() == query_len {
        score += 20;
    }
    if matched_indices.first().copied() == Some(0) {
        score += 30;
    }

    let mut consecutive = 1;
    for pair in matched_indices.windows(2) {
        if pair[1] == pair[0] + 1 {
            consecutive += 1;
            score += 8;
        } else {
            let gap = pair[1].saturating_sub(pair[0] + 1) as i32;
            score -= gap * 2;
        }
    }
    if consecutive == matched_indices.len() {
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
    }

    if candidate_lower.ends_with(query_lower.as_str()) {
        score += 50;
    }

    score - (chars.len() as i32 / 2)
}

fn indices_to_ranges(indices: &[usize]) -> Vec<(usize, usize)> {
    if indices.is_empty() {
        return Vec::new();
    }
    let mut ranges = Vec::<(usize, usize)>::new();
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
