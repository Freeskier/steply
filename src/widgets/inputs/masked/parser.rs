use super::model::{MaskToken, SegmentKind, SegmentRole, SegmentSpec};

pub(super) fn parse_mask(mask: &str) -> Vec<MaskToken> {
    let mut tokens = Vec::<MaskToken>::new();
    let chars: Vec<char> = mask.chars().collect();
    let mut idx = 0usize;

    while idx < chars.len() {
        let ch = chars[idx];
        let short_kind = match ch {
            '#' => Some(SegmentKind::Digit),
            'A' => Some(SegmentKind::Alpha),
            '*' => Some(SegmentKind::Alnum),
            _ => None,
        };

        if let Some(kind) = short_kind {
            let (min_len, max_len, range, next_idx) = parse_quantifier(&chars, idx + 1);
            let resolved_kind = if let Some((min, max)) = range {
                SegmentKind::NumericRange { min, max }
            } else {
                kind
            };

            tokens.push(MaskToken::Segment(SegmentSpec {
                kind: resolved_kind,
                min_len,
                max_len,
                role: None,
                value: String::new(),
            }));
            idx = next_idx;
            continue;
        }

        if ch.is_alphabetic() {
            let start = idx;
            while idx < chars.len() && chars[idx].is_alphabetic() {
                idx += 1;
            }
            let token: String = chars[start..idx].iter().collect();
            if let Some((kind, min_len, max_len, role)) = date_token(token.as_str()) {
                tokens.push(MaskToken::Segment(SegmentSpec {
                    kind,
                    min_len,
                    max_len,
                    role: Some(role),
                    value: String::new(),
                }));
            } else {
                for part in token.chars() {
                    tokens.push(MaskToken::Literal(part));
                }
            }
            continue;
        }

        tokens.push(MaskToken::Literal(ch));
        idx += 1;
    }

    tokens
}

fn date_token(token: &str) -> Option<(SegmentKind, usize, Option<usize>, SegmentRole)> {
    match token {
        "YYYY" => Some((
            SegmentKind::NumericRange {
                min: 1900,
                max: 2100,
            },
            4,
            Some(4),
            SegmentRole::Year,
        )),
        "MM" => Some((
            SegmentKind::NumericRange { min: 1, max: 12 },
            2,
            Some(2),
            SegmentRole::Month,
        )),
        "DD" => Some((
            SegmentKind::NumericRange { min: 1, max: 31 },
            2,
            Some(2),
            SegmentRole::Day,
        )),
        "HH" => Some((
            SegmentKind::NumericRange { min: 0, max: 23 },
            2,
            Some(2),
            SegmentRole::Hour,
        )),
        "mm" => Some((
            SegmentKind::NumericRange { min: 0, max: 59 },
            2,
            Some(2),
            SegmentRole::Minute,
        )),
        "ss" => Some((
            SegmentKind::NumericRange { min: 0, max: 59 },
            2,
            Some(2),
            SegmentRole::Second,
        )),
        _ => None,
    }
}

fn parse_quantifier(
    chars: &[char],
    mut idx: usize,
) -> (usize, Option<usize>, Option<(i64, i64)>, usize) {
    if idx >= chars.len() || chars[idx] != '{' {
        return (1, Some(1), None, idx);
    }
    idx += 1;
    let start = idx;

    while idx < chars.len() && chars[idx] != '}' {
        idx += 1;
    }
    if idx >= chars.len() || chars[idx] != '}' {
        return (1, Some(1), None, start.saturating_sub(1));
    }

    let inner: String = chars[start..idx].iter().collect();
    idx += 1;

    if inner.is_empty() {
        return (0, None, None, idx);
    }

    let mut len_part = inner.as_str();
    let mut range: Option<(i64, i64)> = None;
    if let Some((left, right)) = inner.split_once(':') {
        len_part = left;
        if let Some((min, max)) = right.split_once('-')
            && let (Ok(min), Ok(max)) = (min.parse::<i64>(), max.parse::<i64>())
        {
            range = Some((min, max));
        }
    }

    let (min_len, max_len) = if len_part.contains(',') {
        let parts: Vec<&str> = len_part.split(',').collect();
        let min_len = parts
            .first()
            .and_then(|item| item.parse::<usize>().ok())
            .unwrap_or(0);
        let max_len = parts.get(1).and_then(|item| item.parse::<usize>().ok());
        (min_len, max_len)
    } else {
        let len = len_part.parse::<usize>().unwrap_or(1);
        (len, Some(len))
    };

    (min_len, max_len, range, idx)
}
