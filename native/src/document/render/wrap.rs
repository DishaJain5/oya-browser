/** Owned line breaking preserves UTF-8 boundaries and never delegates page layout. */
use crate::platform::measure;

/** Whitespace-normalized text wraps using the same measured runs later painted. */
pub(super) fn lines(text: &str, width: f64, size: f64, bold: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if measure(&candidate, size, bold) <= width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        let (complete, last) = word_parts(word, width, size, bold);
        lines.extend(complete);
        current = last;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/** Binary-search scalar boundaries bound expensive OS shaping for unusually long words. */
fn word_parts(mut word: &str, width: f64, size: f64, bold: bool) -> (Vec<String>, String) {
    let mut lines = Vec::new();
    while measure(word, size, bold) > width {
        let boundaries = word
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(word.len()))
            .collect::<Vec<_>>();
        let (mut low, mut high) = (1, boundaries.len() - 1);
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            if measure(&word[..boundaries[middle]], size, bold) <= width {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let split = boundaries[low];
        lines.push(word[..split].into());
        word = &word[split..];
    }
    (lines, word.into())
}
