use crate::core::search::fuzzy::match_text;

pub fn clamp_index(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        current.min(len.saturating_sub(1))
    }
}

pub fn cycle_next(current: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some((current + 1) % len)
}

pub fn cycle_prev(current: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some((current + len - 1) % len)
}

pub fn move_by(current: usize, delta: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let next = current.checked_add_signed(delta)?;
    if next >= len {
        return None;
    }
    Some(next)
}

#[derive(Debug, Clone, Copy)]
pub struct FilterField<'a> {
    pub text: &'a str,
    pub boost: i32,
}

pub fn rank_by_filter<'a, T>(
    query: &str,
    records: &'a [T],
    mut fields_for: impl FnMut(&'a T) -> Vec<FilterField<'a>>,
) -> Vec<(usize, Vec<Vec<(usize, usize)>>)> {
    let query = query.trim();
    let mut ranked = Vec::<(usize, i32, Vec<Vec<(usize, usize)>>)>::new();

    for (index, record) in records.iter().enumerate() {
        let fields = fields_for(record);
        let mut score = None::<i32>;
        let mut highlights = Vec::<Vec<(usize, usize)>>::with_capacity(fields.len());

        for field in fields {
            match match_text(query, field.text) {
                Some((field_score, ranges)) => {
                    let weighted = field_score + field.boost;
                    score = Some(score.map_or(weighted, |current| current.max(weighted)));
                    highlights.push(ranges);
                }
                None => highlights.push(Vec::new()),
            }
        }

        if let Some(score) = score {
            ranked.push((index, score, highlights));
        }
    }

    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    ranked
        .into_iter()
        .map(|(index, _, highlights)| (index, highlights))
        .collect()
}

pub fn text_matches(query: &str, text: &str) -> bool {
    match_text(query.trim(), text).is_some()
}

pub fn text_match_ranges(query: &str, text: &str) -> Vec<(usize, usize)> {
    match_text(query.trim(), text)
        .map(|(_, ranges)| ranges)
        .unwrap_or_default()
}
