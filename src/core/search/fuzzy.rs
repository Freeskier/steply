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
            let score = score_match(candidate, &matched_indices);
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

fn score_match(candidate: &str, matched_indices: &[usize]) -> i32 {
    if matched_indices.is_empty() {
        return 0;
    }

    let mut score = matched_indices.len() as i32;

    for pair in matched_indices.windows(2) {
        if pair[1] == pair[0] + 1 {
            score += 3;
        }
    }

    if let Some(first) = matched_indices.first() {
        let len = candidate.chars().count() as i32;
        score += (len - *first as i32).max(0);
    }

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
