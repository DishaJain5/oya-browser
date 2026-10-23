/** Bounded scalar-safe line breaking reuses measured widths without rescanning long suffixes. */
use super::sizing::LayoutContext;
use crate::protocol::EngineResult;

/** Placement consumes the exact width from bounded shaping instead of measuring each chunk again. */
#[derive(Clone)]
pub(super) struct MeasuredText {
    /** Owned text retains every scalar admitted from the source word. */
    pub text: String,
    /** Native width measured with the same font and weight as painting. */
    pub width: f64,
}

/** Whitespace-normalized text accumulates widths without remeasuring a growing merged line. */
pub(super) fn lines(
    text: &str,
    width: f64,
    font: f64,
    bold: bool,
    context: &mut LayoutContext<'_>,
) -> EngineResult<Vec<MeasuredText>> {
    if let Some(lines) = context.wrapped(text, width, font, bold)? {
        return Ok(lines);
    }
    context.charge(text.chars().count() + 1)?;
    let mut result = Vec::new();
    let mut current = MeasuredText {
        text: String::new(),
        width: 0.0,
    };
    let gap = context.measure(" ", font, bold)?;
    for word in text.split_whitespace() {
        let mut chunks = word_parts(word, width, font, bold, context)?
            .into_iter()
            .peekable();
        while let Some(chunk) = chunks.next() {
            append_chunk(&mut result, &mut current, chunk, width, gap);
            if chunks.peek().is_some() {
                result.push(std::mem::replace(
                    &mut current,
                    MeasuredText {
                        text: String::new(),
                        width: 0.0,
                    },
                ));
            }
        }
    }
    if !current.text.is_empty() {
        result.push(current);
    }
    context.remember_wrapped(text, width, font, bold, &result)?;
    Ok(result)
}

/** Combining a measured run copies once; prior line contents are never sent back to the shaper. */
fn append_chunk(
    result: &mut Vec<MeasuredText>,
    current: &mut MeasuredText,
    chunk: MeasuredText,
    width: f64,
    gap: f64,
) {
    if !current.text.is_empty() && current.width + gap + chunk.width <= width {
        current.text.push(' ');
        current.text.push_str(&chunk.text);
        current.width += gap + chunk.width;
        return;
    }
    if !current.text.is_empty() {
        result.push(std::mem::replace(
            current,
            MeasuredText {
                text: String::new(),
                width: 0.0,
            },
        ));
    }
    *current = chunk;
}

/** One boundary table and forward-only indexes avoid repeated whole-suffix scans and shaping. */
fn word_parts(
    word: &str,
    width: f64,
    font: f64,
    bold: bool,
    context: &mut LayoutContext<'_>,
) -> EngineResult<Vec<MeasuredText>> {
    context.charge(word.chars().count() + 1)?;
    let boundaries = word
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(word.len()))
        .collect::<Vec<_>>();
    let mut start = 0;
    let mut hint = 1;
    let mut result = Vec::new();
    while start + 1 < boundaries.len() {
        let (end, measured) =
            fitting_prefix(word, &boundaries, (start, hint), width, font, bold, context)?;
        result.push(MeasuredText {
            text: word[boundaries[start]..boundaries[end]].into(),
            width: measured,
        });
        hint = end - start;
        start = end;
    }
    Ok(result)
}

/** Previous line length seeds local probes; proportional bracketing avoids restarting every scan at one. */
fn fitting_prefix(
    word: &str,
    boundaries: &[usize],
    position: (usize, usize),
    width: f64,
    font: f64,
    bold: bool,
    context: &mut LayoutContext<'_>,
) -> EngineResult<(usize, f64)> {
    let (start, hint) = position;
    let last = boundaries.len() - 1;
    let mut fit = (start + hint).min(last);
    let mut measured = context.measure(&word[boundaries[start]..boundaries[fit]], font, bold)?;
    if measured > width {
        let first = start + 1;
        let minimum = if fit == first {
            measured
        } else {
            context.measure(&word[boundaries[start]..boundaries[first]], font, bold)?
        };
        if minimum > width || fit == first {
            return Ok((first, minimum));
        }
        return search_prefix(
            word,
            boundaries,
            start,
            (first, fit, minimum),
            width,
            (font, bold),
            context,
        );
    }
    let mut step = 1;
    while fit < last {
        let probe = (fit + step).min(last);
        let candidate = context.measure(&word[boundaries[start]..boundaries[probe]], font, bold)?;
        if candidate > width {
            return search_prefix(
                word,
                boundaries,
                start,
                (fit, probe, measured),
                width,
                (font, bold),
                context,
            );
        }
        fit = probe;
        measured = candidate;
        step = step.saturating_mul(2);
    }
    Ok((fit, measured))
}

/** Every binary-search substring is charged through the same context as final placement. */
fn search_prefix(
    word: &str,
    boundaries: &[usize],
    start: usize,
    bracket: (usize, usize, f64),
    width: f64,
    font: (f64, bool),
    context: &mut LayoutContext<'_>,
) -> EngineResult<(usize, f64)> {
    let (mut fit, mut overflow, mut measured) = bracket;
    let (font, bold) = font;
    while fit + 1 < overflow {
        let probe = fit + (overflow - fit) / 2;
        let candidate = context.measure(&word[boundaries[start]..boundaries[probe]], font, bold)?;
        if candidate <= width {
            fit = probe;
            measured = candidate;
        } else {
            overflow = probe;
        }
    }
    Ok((fit, measured))
}
