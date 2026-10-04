//! Fuzzy matching shared by the command palette and file search.

/// Case-insensitive subsequence match. Higher is better; `None` if not all chars match.
/// Rewards matches at word starts and consecutive runs, penalises gaps.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut qi = 0;
    let mut score = 0;
    let mut last: Option<usize> = None;
    for (i, c) in chars.iter().enumerate() {
        if qi == query.len() {
            break;
        }
        if c.to_lowercase().eq(std::iter::once(query[qi])) {
            let word_start = i == 0 || !chars[i - 1].is_alphanumeric();
            score += 10;
            if word_start {
                score += 8;
            }
            match last {
                Some(l) if l + 1 == i => score += 6,
                Some(l) => score -= ((i - l - 1) as i32).min(5),
                None => score -= (i as i32).min(10),
            }
            last = Some(i);
            qi += 1;
        }
    }
    (qi == query.len()).then_some(score)
}

#[cfg(test)]
mod tests {
    use super::score as fuzzy_score;

    #[test]
    fn fuzzy_matching() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
        assert!(fuzzy_score("xyz", "claude").is_none());
        assert!(fuzzy_score("cl", "claude · ~/proj").is_some());
        // Word starts and contiguous runs beat scattered matches.
        assert!(
            fuzzy_score("nt", "New tab").unwrap() > fuzzy_score("nt", "Go to next tab").unwrap()
        );
        assert!(
            fuzzy_score("clo", "Close tab").unwrap()
                > fuzzy_score("clo", "Scroll down one").unwrap()
        );
        assert!(fuzzy_score("CLAUDE", "claude").is_some());
    }
}
