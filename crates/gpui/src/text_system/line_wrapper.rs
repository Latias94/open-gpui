use crate::{Font, FontId, Pixels, SharedString, TextRun, TextSystem, px};
use open_gpui_collections::HashMap;
use std::{borrow::Cow, iter, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;

/// Determines which part of the text is replaced by the truncation affix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TruncateFrom {
    /// Truncate text from the start.
    Start,
    /// Truncate text from the end.
    End,
    /// Truncate text from the middle, preserving the start and end.
    Middle,
}

/// Controls how soft-wrapped continuation lines are indented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IndentAdjustment {
    /// Start continuation lines at column zero.
    NoIndent,
    /// Match the original line's leading spaces.
    #[default]
    SameIndent,
    /// Add this many space-character widths to the original indentation.
    ExtraColumns(u32),
}

/// The GPUI line wrapper, used to wrap lines of text to a given width.
pub struct LineWrapper {
    text_system: Arc<TextSystem>,
    font: Font,
    resolved_font_generation: usize,
    pub(crate) font_id: FontId,
    pub(crate) font_size: Pixels,
    char_widths: CharWidthCache,
    text_buffer: String,
}

/// Cached character widths for one resolved font and size, independent of its TextSystem.
pub(crate) struct CharWidthCache {
    font_generation: Option<usize>,
    ascii: [Option<Pixels>; 128],
    other: HashMap<char, Pixels>,
}

impl Default for CharWidthCache {
    fn default() -> Self {
        Self {
            font_generation: None,
            ascii: [None; 128],
            other: HashMap::default(),
        }
    }
}

impl LineWrapper {
    /// The maximum indent that can be applied to a line.
    pub const MAX_INDENT: u32 = 256;

    pub(crate) fn new(
        font: Font,
        font_id: FontId,
        resolved_font_generation: usize,
        font_size: Pixels,
        text_system: Arc<TextSystem>,
        char_widths: CharWidthCache,
    ) -> Self {
        Self {
            text_system,
            font,
            resolved_font_generation,
            font_id,
            font_size,
            char_widths,
            text_buffer: String::new(),
        }
    }

    pub(crate) fn into_char_widths(self) -> CharWidthCache {
        self.char_widths
    }

    /// Wrap a line of text to the given width with this wrapper's font and font size.
    ///
    /// Break opportunities and extended graphemes span adjacent fragments. Inline elements stay
    /// atomic and contribute their declared byte lengths. A grapheme wider than the available
    /// width stays whole. `indent_adjustment` controls continuation rows.
    pub fn wrap_line<'a>(
        &'a mut self,
        fragments: &'a [LineFragment],
        wrap_width: Pixels,
        indent_adjustment: IndentAdjustment,
    ) -> impl Iterator<Item = Boundary> + 'a {
        let mut width = px(0.);
        let mut first_non_whitespace_ix = None;
        let mut base_indent = None;
        let mut last_candidate_ix = 0;
        let mut last_candidate_width = px(0.);
        let mut last_wrap_ix = 0;
        let mut index = 0;
        let Self {
            text_buffer: text,
            char_widths,
            text_system,
            font,
            font_id,
            resolved_font_generation,
            font_size,
        } = self;
        text.clear();
        for fragment in fragments {
            match fragment {
                LineFragment::Text {
                    text: fragment_text,
                } => text.push_str(fragment_text),
                LineFragment::Element { len_utf8, .. } => {
                    text.extend(iter::repeat_n('a', *len_utf8));
                }
            }
        }
        let mut break_indices = Self::break_indices(text).peekable();
        let mut grapheme_indices = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .peekable();
        let mut width_for_char = move |character| {
            width_for_char(
                char_widths,
                character,
                text_system,
                font,
                font_id,
                resolved_font_generation,
                *font_size,
            )
        };
        let mut grapheme_index = 0;
        let mut width_before_grapheme = px(0.);
        let mut candidates = fragments
            .iter()
            .flat_map(move |fragment| fragment.wrap_boundary_candidates())
            .peekable();
        iter::from_fn(move || {
            for candidate in candidates.by_ref() {
                let ix = index;
                index += candidate.len_utf8();
                while grapheme_indices
                    .next_if(|position| *position < ix)
                    .is_some()
                {}
                let is_grapheme_boundary = grapheme_indices.next_if_eq(&ix).is_some();
                if is_grapheme_boundary {
                    grapheme_index = ix;
                    width_before_grapheme = width;
                }
                if break_indices.next_if_eq(&ix).is_some()
                    && is_grapheme_boundary
                    && first_non_whitespace_ix.is_some()
                {
                    last_candidate_ix = ix;
                    last_candidate_width = width;
                }
                let item_width = match candidate {
                    WrapBoundaryCandidate::Char { character: c } => {
                        if c == '\n' {
                            continue;
                        }

                        if c != ' ' && first_non_whitespace_ix.is_none() {
                            first_non_whitespace_ix = Some(ix);
                        }

                        width_for_char(c)
                    }
                    WrapBoundaryCandidate::Element {
                        width: element_width,
                        ..
                    } => {
                        if first_non_whitespace_ix.is_none() {
                            first_non_whitespace_ix = Some(ix);
                        }

                        element_width
                    }
                };

                width += item_width;
                if width > wrap_width && grapheme_index > last_wrap_ix {
                    let wrap_at_candidate = last_candidate_ix > last_wrap_ix
                        && width - last_candidate_width <= wrap_width;
                    let carried_width = if wrap_at_candidate {
                        width - last_candidate_width
                    } else {
                        width - width_before_grapheme
                    };
                    if base_indent.is_none() {
                        if let Some(first_non_whitespace_ix) = first_non_whitespace_ix {
                            base_indent =
                                Some(Self::MAX_INDENT.min(
                                    first_non_whitespace_ix.saturating_sub(last_wrap_ix) as u32,
                                ));
                        }
                    }
                    let next_indent = match indent_adjustment {
                        IndentAdjustment::NoIndent => 0,
                        IndentAdjustment::SameIndent => base_indent.unwrap_or(0),
                        IndentAdjustment::ExtraColumns(extra) => {
                            if let Some(base_indent) = base_indent {
                                let candidate = base_indent.saturating_add(extra);
                                let indent_width = width_for_char(' ') * candidate as f32;
                                let headroom = carried_width.max(width_for_char(' ') * 2.);
                                if indent_width + headroom > wrap_width {
                                    0
                                } else {
                                    Self::MAX_INDENT.min(candidate)
                                }
                            } else {
                                0
                            }
                        }
                    };
                    if wrap_at_candidate {
                        last_wrap_ix = last_candidate_ix;
                        width -= last_candidate_width;
                        width_before_grapheme -= last_candidate_width;
                    } else {
                        last_wrap_ix = grapheme_index;
                        width -= width_before_grapheme;
                        width_before_grapheme = px(0.);
                    }
                    last_candidate_ix = 0;
                    let indent_width = width_for_char(' ') * next_indent as f32;
                    width += indent_width;
                    width_before_grapheme += indent_width;
                    return Some(Boundary::new(last_wrap_ix, next_indent));
                }
            }

            None
        })
    }

    /// Determines if a line should be truncated based on its width.
    ///
    /// Returns the truncation index in `line` for start or end truncation.
    /// Middle truncation needs two boundaries; use [`Self::truncate_line`] instead.
    pub fn should_truncate_line(
        &mut self,
        line: &str,
        truncate_width: Pixels,
        truncation_affix: &str,
        truncate_from: TruncateFrom,
    ) -> Option<usize> {
        let mut width = px(0.);
        let suffix_width = truncation_affix
            .chars()
            .map(|c| self.width_for_char(c))
            .fold(px(0.0), |a, x| a + x);
        let mut truncate_ix = if truncate_from == TruncateFrom::Start {
            line.grapheme_indices(true)
                .next_back()
                .map_or(0, |(index, _)| index)
        } else {
            0
        };

        match truncate_from {
            TruncateFrom::Start => {
                for (ix, grapheme) in line.grapheme_indices(true).rev() {
                    if width + suffix_width < truncate_width {
                        truncate_ix = ix;
                    }

                    width += grapheme
                        .chars()
                        .map(|c| self.width_for_char(c))
                        .sum::<Pixels>();

                    if width.floor() > truncate_width {
                        return Some(truncate_ix);
                    }
                }
            }
            TruncateFrom::End => {
                for (ix, grapheme) in line.grapheme_indices(true) {
                    if width + suffix_width < truncate_width {
                        truncate_ix = ix;
                    }

                    width += grapheme
                        .chars()
                        .map(|c| self.width_for_char(c))
                        .sum::<Pixels>();

                    if width.floor() > truncate_width {
                        return Some(truncate_ix);
                    }
                }
            }
            TruncateFrom::Middle => {}
        }

        None
    }

    fn should_truncate_line_middle(
        &mut self,
        line: &str,
        truncate_width: Pixels,
        truncation_affix: &str,
        single_paragraph: bool,
    ) -> Option<(usize, usize)> {
        let affix_width: Pixels = truncation_affix
            .chars()
            .map(|c| self.width_for_char(c))
            .sum();
        let total_width: Pixels = line.chars().map(|c| self.width_for_char(c)).sum();
        let paragraph_bounds = if single_paragraph {
            let mut newlines = line
                .grapheme_indices(true)
                .filter(|(_, grapheme)| grapheme.contains('\n'));
            newlines.next().map(|first| {
                let last = newlines.next_back().unwrap_or(first);
                (first.0, last.0 + last.1.len())
            })
        } else {
            None
        };
        if let Some((front, back)) = paragraph_bounds {
            let ends_width: Pixels = line[..front]
                .chars()
                .chain(line[back..].chars())
                .map(|c| self.width_for_char(c))
                .sum();
            if ends_width + affix_width <= truncate_width {
                return Some((front, back));
            }
        } else if total_width <= truncate_width {
            return None;
        }
        let content_budget = truncate_width - affix_width;
        if content_budget <= px(0.) {
            return Some((0, line.len()));
        }
        let front_budget = content_budget * (2. / 3.);
        let back_budget = content_budget - front_budget;
        let mut front_width = px(0.);
        let mut front_end_ix = 0;
        for (ix, grapheme) in line.grapheme_indices(true) {
            if paragraph_bounds.is_some_and(|(front, _)| ix >= front) {
                break;
            }
            let width: Pixels = grapheme.chars().map(|c| self.width_for_char(c)).sum();
            if front_width + width > front_budget {
                break;
            }
            front_width += width;
            front_end_ix = ix + grapheme.len();
        }
        let mut back_width = px(0.);
        let mut back_start_ix = line.len();
        for (ix, grapheme) in line.grapheme_indices(true).rev() {
            if paragraph_bounds.is_some_and(|(_, back)| ix < back) {
                break;
            }
            let width: Pixels = grapheme.chars().map(|c| self.width_for_char(c)).sum();
            if back_width + width > back_budget {
                break;
            }
            back_width += width;
            back_start_ix = ix;
        }
        if front_end_ix >= back_start_ix {
            return Some((0, line.len()));
        }
        Some((front_end_ix, back_start_ix))
    }

    fn truncate_middle<'a>(
        &mut self,
        line: SharedString,
        width: Pixels,
        affix: &str,
        runs: &'a [TextRun],
        single_paragraph: bool,
    ) -> (SharedString, Cow<'a, [TextRun]>) {
        if let Some((front, back)) =
            self.should_truncate_line_middle(&line, width, affix, single_paragraph)
        {
            let result = SharedString::from(format!("{}{affix}{}", &line[..front], &line[back..]));
            let mut runs = runs.to_vec();
            update_runs_after_middle_truncation(affix, &mut runs, front, back);
            (result, Cow::Owned(runs))
        } else {
            (line, Cow::Borrowed(runs))
        }
    }

    /// Truncate a line of text to the given width with this wrapper's font and font size.
    pub fn truncate_line<'a>(
        &mut self,
        line: SharedString,
        truncate_width: Pixels,
        truncation_affix: &str,
        runs: &'a [TextRun],
        truncate_from: TruncateFrom,
    ) -> (SharedString, Cow<'a, [TextRun]>) {
        if truncate_from == TruncateFrom::Middle {
            return self.truncate_middle(line, truncate_width, truncation_affix, runs, false);
        }
        if let Some(truncate_ix) =
            self.should_truncate_line(&line, truncate_width, truncation_affix, truncate_from)
        {
            let result = match truncate_from {
                TruncateFrom::Start => SharedString::from(format!(
                    "{truncation_affix}{}",
                    &line[truncate_ix
                        + line[truncate_ix..]
                            .graphemes(true)
                            .next()
                            .map_or(0, str::len)..]
                )),
                TruncateFrom::End => SharedString::from(format!(
                    "{}{truncation_affix}",
                    trim_truncation_end(&line[..truncate_ix])
                )),
                TruncateFrom::Middle => unreachable!("middle truncation is handled above"),
            };
            let mut runs = runs.to_vec();
            update_runs_after_truncation(&result, truncation_affix, &mut runs, truncate_from);
            (result, Cow::Owned(runs))
        } else {
            (line, Cow::Borrowed(runs))
        }
    }

    /// Truncate text to fit within a given number of wrapped lines.
    ///
    /// Start and End truncation measure complete-grapheme candidates through the same wrapper and
    /// paragraph contents used for layout, reserving the affix within the row budget. Empty trailing
    /// paragraphs may be omitted without an affix. Middle truncation keeps the first and last
    /// paragraph ends together on one row. The complete affix stays visible even if it exceeds the
    /// available width by itself.
    ///
    /// Flat single-row input retains [`Self::truncate_line`]'s width policy.
    pub fn truncate_wrapped_line<'a>(
        &mut self,
        text: SharedString,
        wrap_width: Pixels,
        max_lines: usize,
        truncation_affix: &str,
        runs: &'a [TextRun],
        truncate_from: TruncateFrom,
    ) -> (SharedString, Cow<'a, [TextRun]>) {
        if max_lines == 0 {
            return self.truncate_line(
                text,
                wrap_width * max_lines,
                truncation_affix,
                runs,
                truncate_from,
            );
        }
        if truncate_from == TruncateFrom::Middle {
            return self.truncate_middle(text, wrap_width, truncation_affix, runs, true);
        }
        if max_lines == 1 && !text.contains('\n') {
            return self.truncate_line(text, wrap_width, truncation_affix, runs, truncate_from);
        }
        if self.fits_wrapped_lines(&text, wrap_width, max_lines) {
            return (text, Cow::Borrowed(runs));
        }
        let content_end = text
            .grapheme_indices(true)
            .rev()
            .find(|(_, grapheme)| !matches!(*grapheme, "\n" | "\r\n"))
            .map_or(0, |(index, grapheme)| index + grapheme.len());
        if content_end < text.len()
            && self.fits_wrapped_lines(&text[..content_end], wrap_width, max_lines)
        {
            let result: SharedString = text[..content_end].to_owned().into();
            let mut runs = runs.to_vec();
            update_runs_after_truncation(&result, "", &mut runs, TruncateFrom::End);
            return (result, Cow::Owned(runs));
        }
        let boundaries = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(iter::once(text.len()))
            .collect::<Vec<_>>();
        let from_start = truncate_from == TruncateFrom::Start;
        let mut lower = usize::from(from_start);
        let mut upper = boundaries.len() - if from_start { 1 } else { 2 };
        let write_candidate = |result: &mut String, boundary: usize| {
            result.clear();
            if from_start {
                result.push_str(truncation_affix);
                result.push_str(&text[boundary..]);
            } else {
                result.push_str(trim_truncation_end(&text[..boundary]));
                result.push_str(truncation_affix);
            }
        };
        let mut result = String::with_capacity(text.len() + truncation_affix.len());
        while lower < upper {
            let midpoint = lower + (upper - lower + usize::from(!from_start)) / 2;
            write_candidate(&mut result, boundaries[midpoint]);
            if self.fits_wrapped_lines(&result, wrap_width, max_lines) {
                if from_start {
                    upper = midpoint;
                } else {
                    lower = midpoint;
                }
            } else if from_start {
                lower = midpoint + 1;
            } else {
                upper = midpoint - 1;
            }
        }
        write_candidate(&mut result, boundaries[lower]);
        let mut runs = runs.to_vec();
        update_runs_after_truncation(&result, truncation_affix, &mut runs, truncate_from);
        (result.into(), Cow::Owned(runs))
    }

    fn fits_wrapped_lines(&mut self, text: &str, wrap_width: Pixels, max_lines: usize) -> bool {
        let mut remaining = max_lines;
        for line in text.split('\n') {
            if remaining == 0 {
                return false;
            }
            let fragments = [LineFragment::text(line)];
            let wraps = self
                .wrap_line(&fragments, wrap_width, IndentAdjustment::SameIndent)
                .take(remaining)
                .count();
            if wraps >= remaining {
                return false;
            }
            remaining -= wraps + 1;
        }
        true
    }

    /// Byte offsets at which Unicode line breaking allows a new line.
    pub(crate) fn break_indices(text: &str) -> impl Iterator<Item = usize> {
        icu_segmenter::LineSegmenter::new_for_non_complex_scripts(Default::default())
            .segment_str(text)
    }

    #[inline(always)]
    fn width_for_char(&mut self, c: char) -> Pixels {
        width_for_char(
            &mut self.char_widths,
            c,
            &self.text_system,
            &self.font,
            &mut self.font_id,
            &mut self.resolved_font_generation,
            self.font_size,
        )
    }
}

#[inline(always)]
fn width_for_char(
    char_widths: &mut CharWidthCache,
    c: char,
    text_system: &TextSystem,
    font: &Font,
    font_id: &mut FontId,
    resolved_font_generation: &mut usize,
    font_size: Pixels,
) -> Pixels {
    let generation = text_system.font_generation();
    if *resolved_font_generation != generation {
        *font_id = text_system.resolve_font(font);
        *resolved_font_generation = generation;
        *char_widths = CharWidthCache::default();
    }
    if char_widths.font_generation != Some(generation) {
        *char_widths = CharWidthCache {
            font_generation: Some(generation),
            ..Default::default()
        };
    }
    if (c as u32) < 128 {
        if let Some(cached_width) = char_widths.ascii[c as usize] {
            cached_width
        } else {
            let width = text_system.layout_width(*font_id, font_size, c);
            char_widths.ascii[c as usize] = Some(width);
            width
        }
    } else if let Some(cached_width) = char_widths.other.get(&c) {
        *cached_width
    } else {
        let width = text_system.layout_width(*font_id, font_size, c);
        char_widths.other.insert(c, width);
        width
    }
}

fn trim_truncation_end(text: &str) -> &str {
    let end = text
        .grapheme_indices(true)
        .rev()
        .find(|(_, grapheme)| {
            !grapheme
                .chars()
                .all(|c| c.is_whitespace() || c.is_ascii_punctuation())
        })
        .map_or(0, |(index, grapheme)| index + grapheme.len());
    &text[..end]
}

fn update_runs_after_truncation(
    result: &str,
    ellipsis: &str,
    runs: &mut Vec<TextRun>,
    truncate_from: TruncateFrom,
) {
    let mut truncate_at = result.len() - ellipsis.len();
    match truncate_from {
        TruncateFrom::Start => {
            for (run_index, run) in runs.iter_mut().enumerate().rev() {
                if run.len <= truncate_at {
                    truncate_at -= run.len;
                } else {
                    run.len = truncate_at + ellipsis.len();
                    runs.splice(..run_index, std::iter::empty());
                    break;
                }
            }
        }
        TruncateFrom::End => {
            for (run_index, run) in runs.iter_mut().enumerate() {
                if run.len <= truncate_at {
                    truncate_at -= run.len;
                } else {
                    run.len = truncate_at + ellipsis.len();
                    runs.truncate(run_index + 1);
                    break;
                }
            }
        }
        TruncateFrom::Middle => unreachable!("middle truncation updates both retained ranges"),
    }
}

fn update_runs_after_middle_truncation(
    ellipsis: &str,
    runs: &mut Vec<TextRun>,
    front_end_ix: usize,
    back_start_ix: usize,
) {
    let original_runs = std::mem::take(runs);
    let mut result = Vec::with_capacity(original_runs.len());
    let mut front_remaining = front_end_ix;
    let mut front_done = false;
    for run in &original_runs {
        if front_done {
            break;
        }
        if run.len <= front_remaining {
            result.push(run.clone());
            front_remaining -= run.len;
        } else {
            let mut partial = run.clone();
            partial.len = front_remaining + ellipsis.len();
            result.push(partial);
            front_done = true;
        }
    }
    if !front_done {
        if let Some(last) = result.last_mut() {
            last.len += ellipsis.len();
        } else if let Some(first) = original_runs.first() {
            let mut affix_run = first.clone();
            affix_run.len = ellipsis.len();
            result.push(affix_run);
        }
    }
    let mut byte_pos = 0;
    for run in &original_runs {
        let run_end = byte_pos + run.len;
        if run_end > back_start_ix {
            let mut partial = run.clone();
            if byte_pos < back_start_ix {
                partial.len = run_end - back_start_ix;
            }
            result.push(partial);
        }
        byte_pos = run_end;
    }
    *runs = result;
}

/// A fragment of a line that can be wrapped.
pub enum LineFragment<'a> {
    /// A text fragment consisting of characters.
    Text {
        /// The text content of the fragment.
        text: &'a str,
    },
    /// A non-text element with a fixed width.
    Element {
        /// The width of the element in pixels.
        width: Pixels,
        /// The UTF-8 encoded length of the element.
        len_utf8: usize,
    },
}

impl<'a> LineFragment<'a> {
    /// Creates a new text fragment from the given text.
    pub fn text(text: &'a str) -> Self {
        LineFragment::Text { text }
    }

    /// Creates a new non-text element with the given width and UTF-8 encoded length.
    pub fn element(width: Pixels, len_utf8: usize) -> Self {
        LineFragment::Element { width, len_utf8 }
    }

    fn wrap_boundary_candidates(&self) -> impl Iterator<Item = WrapBoundaryCandidate> {
        let text = match self {
            LineFragment::Text { text } => text,
            LineFragment::Element { .. } => "\0",
        };
        text.chars().map(move |character| {
            if let LineFragment::Element { width, len_utf8 } = self {
                WrapBoundaryCandidate::Element {
                    width: *width,
                    len_utf8: *len_utf8,
                }
            } else {
                WrapBoundaryCandidate::Char { character }
            }
        })
    }
}

enum WrapBoundaryCandidate {
    Char { character: char },
    Element { width: Pixels, len_utf8: usize },
}

impl WrapBoundaryCandidate {
    pub fn len_utf8(&self) -> usize {
        match self {
            WrapBoundaryCandidate::Char { character } => character.len_utf8(),
            WrapBoundaryCandidate::Element { len_utf8: len, .. } => *len,
        }
    }
}

/// A boundary between two lines of text.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// The UTF-8 byte index at the start of the next line.
    pub ix: usize,
    /// The indent of the next line.
    pub next_indent: u32,
}

impl Boundary {
    fn new(ix: usize, next_indent: u32) -> Self {
        Self { ix, next_indent }
    }
}

#[cfg(test)]
mod tests {
    mod unicode;

    use super::*;
    use crate::{Font, FontFeatures, FontStyle, FontWeight, TestAppContext, TestDispatcher, font};
    #[cfg(target_os = "macos")]
    use crate::{TextRun, WindowTextSystem, WrapBoundary};

    fn build_wrapper() -> LineWrapper {
        let dispatcher = TestDispatcher::new(0);
        let cx = TestAppContext::build(dispatcher, None);
        let id = cx.text_system().resolve_font(&font(".OpenGpuiMono"));
        LineWrapper::new(
            font(".OpenGpuiMono"),
            id,
            cx.text_system().font_generation(),
            px(16.),
            cx.text_system().clone(),
            CharWidthCache::default(),
        )
    }

    fn generate_test_runs(input_run_len: &[usize]) -> Vec<TextRun> {
        input_run_len
            .iter()
            .map(|run_len| TextRun {
                len: *run_len,
                font: Font {
                    family: "Dummy".into(),
                    features: FontFeatures::default(),
                    fallbacks: None,
                    weight: FontWeight::default(),
                    style: FontStyle::Normal,
                },
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn line_breaks_follow_solidus() {
        let mut wrapper = build_wrapper();
        let boundaries = wrapper
            .wrap_line(
                &[LineFragment::text("aaa/bbbb")],
                px(60.),
                IndentAdjustment::default(),
            )
            .collect::<Vec<_>>();
        assert_eq!(boundaries, [Boundary::new(4, 0)]);
    }

    #[test]
    fn wrapping_preserves_extended_graphemes() {
        let mut wrapper = build_wrapper();
        let boundaries = wrapper
            .wrap_line(
                &[LineFragment::text("e\u{301}e\u{301}")],
                px(10.),
                IndentAdjustment::default(),
            )
            .collect::<Vec<_>>();
        assert_eq!(boundaries, [Boundary::new(3, 0)]);
    }

    #[test]
    fn test_wrap_line() {
        let mut wrapper = build_wrapper();

        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("aa bbb cccc ddddd eeee")],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(12, 0),
                Boundary::new(18, 0)
            ],
        );
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("aaa aaaaaaaaaaaaaaaaaa")],
                    px(72.0),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(4, 0),
                Boundary::new(11, 0),
                Boundary::new(18, 0)
            ],
        );
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("     aaaaaaa")],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 5),
                Boundary::new(9, 5),
                Boundary::new(11, 5),
            ]
        );
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("                            ")],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(14, 0),
                Boundary::new(21, 0)
            ]
        );
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("          aaaaaaaaaaaaaa")],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(14, 3),
                Boundary::new(18, 3),
                Boundary::new(22, 3),
            ]
        );

        // Test wrapping multiple text fragments
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::text("aa bbb "),
                        LineFragment::text("cccc ddddd eeee")
                    ],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(12, 0),
                Boundary::new(18, 0)
            ],
        );

        // Test wrapping with a mix of text and element fragments
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::text("aa "),
                        LineFragment::element(px(20.), 1),
                        LineFragment::text(" bbb "),
                        LineFragment::element(px(30.), 1),
                        LineFragment::text(" cccc")
                    ],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(5, 0),
                Boundary::new(9, 0),
                Boundary::new(11, 0)
            ],
        );

        // Test with element at the beginning and text afterward
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::element(px(50.), 1),
                        LineFragment::text(" aaaa bbbb cccc dddd")
                    ],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(2, 0),
                Boundary::new(7, 0),
                Boundary::new(12, 0),
                Boundary::new(17, 0)
            ],
        );

        // Test with a large element that forces wrapping by itself
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::text("short text "),
                        LineFragment::element(px(100.), 1),
                        LineFragment::text(" more text")
                    ],
                    px(72.),
                    IndentAdjustment::default()
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(6, 0),
                Boundary::new(11, 0),
                Boundary::new(12, 0),
                Boundary::new(18, 0)
            ],
        );
    }

    #[test]
    fn test_truncate_line_end() {
        let mut wrapper = build_wrapper();

        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &'static str,
            ellipsis: &str,
        ) {
            let dummy_run_lens = vec![text.len()];
            let dummy_runs = generate_test_runs(&dummy_run_lens);
            let (result, dummy_runs) = wrapper.truncate_line(
                text.into(),
                px(220.),
                ellipsis,
                &dummy_runs,
                TruncateFrom::End,
            );
            assert_eq!(result, expected);
            assert_eq!(dummy_runs.first().unwrap().len, result.len());
        }

        perform_test(
            &mut wrapper,
            "aa bbb cccc ddddd eeee ffff gggg",
            "aa bbb cccc ddddd eeee",
            "",
        );
        perform_test(
            &mut wrapper,
            "aa bbb cccc ddddd eeee ffff gggg",
            "aa bbb cccc ddddd eee…",
            "…",
        );
        perform_test(
            &mut wrapper,
            "aa bbb cccc ddddd eeee ffff gggg",
            "aa bbb cccc dddd......",
            "......",
        );
        perform_test(
            &mut wrapper,
            "aa bbb cccc 🦀🦀🦀🦀🦀 eeee ffff gggg",
            "aa bbb cccc 🦀🦀🦀🦀…",
            "…",
        );
    }

    #[test]
    fn test_truncate_line_start() {
        let mut wrapper = build_wrapper();

        #[track_caller]
        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &'static str,
            ellipsis: &str,
        ) {
            let dummy_run_lens = vec![text.len()];
            let dummy_runs = generate_test_runs(&dummy_run_lens);
            let (result, dummy_runs) = wrapper.truncate_line(
                text.into(),
                px(220.),
                ellipsis,
                &dummy_runs,
                TruncateFrom::Start,
            );
            assert_eq!(result, expected);
            assert_eq!(dummy_runs.first().unwrap().len, result.len());
        }

        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc ddddd eeee fff gg",
            "cccc ddddd eeee fff gg",
            "",
        );
        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc ddddd eeee fff gg",
            "…ccc ddddd eeee fff gg",
            "…",
        );
        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc ddddd eeee fff gg",
            "......dddd eeee fff gg",
            "......",
        );
        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc 🦀🦀🦀🦀🦀 eeee fff gg",
            "…🦀🦀🦀🦀 eeee fff gg",
            "…",
        );
    }

    #[test]
    fn test_truncate_multiple_runs_end() {
        let mut wrapper = build_wrapper();

        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &str,
            run_lens: &[usize],
            result_run_len: &[usize],
            line_width: Pixels,
        ) {
            let dummy_runs = generate_test_runs(run_lens);
            let (result, dummy_runs) =
                wrapper.truncate_line(text.into(), line_width, "…", &dummy_runs, TruncateFrom::End);
            assert_eq!(result, expected);
            for (run, result_len) in dummy_runs.iter().zip(result_run_len) {
                assert_eq!(run.len, *result_len);
            }
        }
        // Case 0: Normal
        // Text: abcdefghijkl
        // Runs: Run0 { len: 12, ... }
        //
        // Truncate res: abcd… (truncate_at = 4)
        // Run res: Run0 { string: abcd…, len: 7, ... }
        perform_test(&mut wrapper, "abcdefghijkl", "abcd…", &[12], &[7], px(50.));
        // Case 1: Drop some runs
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdef… (truncate_at = 6)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: ef…, len:
        // 5, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "abcdef…",
            &[4, 4, 4],
            &[4, 5],
            px(70.),
        );
        // Case 2: Truncate at start of some run
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdefgh… (truncate_at = 8)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: efgh, len:
        // 4, ... }, Run2 { string: …, len: 3, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "abcdefgh…",
            &[4, 4, 4],
            &[4, 4, 3],
            px(90.),
        );
    }

    #[test]
    fn test_truncate_multiple_runs_start() {
        let mut wrapper = build_wrapper();

        #[track_caller]
        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &str,
            run_lens: &[usize],
            result_run_len: &[usize],
            line_width: Pixels,
        ) {
            let dummy_runs = generate_test_runs(run_lens);
            let (result, dummy_runs) = wrapper.truncate_line(
                text.into(),
                line_width,
                "…",
                &dummy_runs,
                TruncateFrom::Start,
            );
            assert_eq!(result, expected);
            for (run, result_len) in dummy_runs.iter().zip(result_run_len) {
                assert_eq!(run.len, *result_len);
            }
        }
        // Case 0: Normal
        // Text: abcdefghijkl
        // Runs: Run0 { len: 12, ... }
        //
        // Truncate res: …ijkl (truncate_at = 9)
        // Run res: Run0 { string: …ijkl, len: 7, ... }
        perform_test(&mut wrapper, "abcdefghijkl", "…ijkl", &[12], &[7], px(50.));
        // Case 1: Drop some runs
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: …ghijkl (truncate_at = 7)
        // Runs res: Run0 { string: …gh, len: 5, ... }, Run1 { string: ijkl, len:
        // 4, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "…ghijkl",
            &[4, 4, 4],
            &[5, 4],
            px(70.),
        );
        // Case 2: Truncate at start of some run
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdefgh… (truncate_at = 3)
        // Runs res: Run0 { string: …, len: 3, ... }, Run1 { string: efgh, len:
        // 4, ... }, Run2 { string: ijkl, len: 4, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "…efghijkl",
            &[4, 4, 4],
            &[3, 4, 4],
            px(90.),
        );
    }

    #[test]
    fn test_update_run_after_truncation_end() {
        fn perform_test(result: &str, run_lens: &[usize], result_run_lens: &[usize]) {
            let mut dummy_runs = generate_test_runs(run_lens);
            update_runs_after_truncation(result, "…", &mut dummy_runs, TruncateFrom::End);
            for (run, result_len) in dummy_runs.iter().zip(result_run_lens) {
                assert_eq!(run.len, *result_len);
            }
        }
        // Case 0: Normal
        // Text: abcdefghijkl
        // Runs: Run0 { len: 12, ... }
        //
        // Truncate res: abcd… (truncate_at = 4)
        // Run res: Run0 { string: abcd…, len: 7, ... }
        perform_test("abcd…", &[12], &[7]);
        // Case 1: Drop some runs
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdef… (truncate_at = 6)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: ef…, len:
        // 5, ... }
        perform_test("abcdef…", &[4, 4, 4], &[4, 5]);
        // Case 2: Truncate at start of some run
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdefgh… (truncate_at = 8)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: efgh, len:
        // 4, ... }, Run2 { string: …, len: 3, ... }
        perform_test("abcdefgh…", &[4, 4, 4], &[4, 4, 3]);
    }

    #[test]
    fn unicode_breaks_preserve_nonseparable_words() {
        for word in [
            "Hello123",
            "var_name",
            "123456",
            "3.1415",
            "100%",
            "@mention",
            "#hashtag",
            "github.com",
            "won’t",
            "‘twas",
            "ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏ",
            "ĀāĂăĄąĆćĈĉĊċČčĎď",
            "АБВГДЕЖЗИЙКЛМНОП",
            "ThậmchíđếnkhithuachạychúngcònnhẫntâmgiếtnốtsốđôngtùchínhtrịởYênBáivàCaoBằng",
            "গিয়েছিলেন",
            "ছেলে",
            "হচ্ছিল",
        ] {
            assert_eq!(
                LineWrapper::break_indices(word).collect::<Vec<_>>(),
                [0, word.len()],
                "{word:?}"
            );
        }
    }

    // These seem to vary wildly based on the text system.
    #[cfg(target_os = "macos")]
    #[crate::test]
    fn test_wrap_shaped_line(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let text_system = WindowTextSystem::new(cx.text_system().clone());

            let normal = TextRun {
                len: 0,
                font: font("Helvetica"),
                color: Default::default(),
                underline: Default::default(),
                ..Default::default()
            };
            let bold = TextRun {
                len: 0,
                font: font("Helvetica").bold(),
                ..Default::default()
            };

            let text = "aa bbb cccc ddddd eeee".into();
            let lines = text_system
                .shape_text(
                    text,
                    px(16.),
                    &[
                        normal.with_len(4),
                        bold.with_len(5),
                        normal.with_len(6),
                        bold.with_len(1),
                        normal.with_len(7),
                    ],
                    Some(px(72.)),
                    None,
                )
                .unwrap();

            assert_eq!(
                lines[0].layout.wrap_boundaries(),
                &[
                    WrapBoundary {
                        run_ix: 0,
                        glyph_ix: 7
                    },
                    WrapBoundary {
                        run_ix: 0,
                        glyph_ix: 12
                    },
                    WrapBoundary {
                        run_ix: 0,
                        glyph_ix: 18
                    }
                ],
            );
        });
    }

    #[test]
    fn test_multiline_truncation_fits_within_wrapped_lines() {
        let mut wrapper = build_wrapper();

        // With .OpenGpuiMono at 16px, each char is 9.6px wide.
        // wrap_width = 72px fits ~7 chars per line.
        //
        // "aa bbbbbb cccccc dddddd eeee ffff" with wrap_width=72px wraps as:
        //   Line 1: "aa "       (28.8px, wraps because "bbbbbb" won't fit)
        //   Line 2: "bbbbbb "   (67.2px)
        //   Line 3: "cccccc "   (67.2px)
        //   ...
        //
        // truncate_wrapped_line should wrap first to find line 2 starts at
        // "bbbbbb...", then truncate only that line to fit with ellipsis.
        let text: &str = "aa bbbbbb cccccc dddddd eeee ffff";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        // The truncated text, when wrapped, must fit within max_lines lines.
        let wrap_count = wrapper
            .wrap_line(
                &[LineFragment::text(&truncated)],
                wrap_width,
                IndentAdjustment::default(),
            )
            .count();

        assert!(
            wrap_count < max_lines,
            "Truncated text '{}' wraps into {} visual lines, expected at most {}",
            truncated,
            wrap_count + 1,
            max_lines
        );

        // The truncated text should end with the ellipsis.
        assert!(
            truncated.ends_with('\u{2026}'),
            "Truncated text '{}' should end with ellipsis",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_no_truncation_needed() {
        let mut wrapper = build_wrapper();

        // Text that fits in 2 lines shouldn't be truncated.
        // Line 1: "aa bbb " (67.2px), Line 2: "cccccc" (57.6px)
        let text: &str = "aa bbb cccccc";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (result, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        assert_eq!(
            result.as_ref(),
            text,
            "Text that fits should not be modified"
        );
    }

    #[test]
    fn test_multiline_truncation_three_lines() {
        let mut wrapper = build_wrapper();

        let text: &str = "aa bbb cccc ddddd eeee ffff gggg hhhh iiii jjjj";
        let wrap_width = px(72.);
        let max_lines: usize = 3;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        let wrap_count = wrapper
            .wrap_line(
                &[LineFragment::text(&truncated)],
                wrap_width,
                IndentAdjustment::default(),
            )
            .count();

        assert!(
            wrap_count < max_lines,
            "Truncated text '{}' wraps into {} visual lines, expected at most {}",
            truncated,
            wrap_count + 1,
            max_lines
        );

        assert!(
            truncated.ends_with('\u{2026}'),
            "Truncated text '{}' should end with ellipsis",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_with_newlines() {
        let mut wrapper = build_wrapper();

        // "hello\nworld foo bar baz" with line_clamp(2):
        // shape_text splits on \n, giving physical lines "hello" and
        // "world foo bar baz". The newline consumes line 1, so the
        // second physical line should be truncated on line 2.
        let text: &str = "hello\nworld foo bar baz";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        // The newline should be preserved.
        let parts: Vec<&str> = truncated.splitn(2, '\n').collect();
        assert_eq!(
            parts.len(),
            2,
            "Newline should be preserved: '{}'",
            truncated
        );
        assert_eq!(parts[0], "hello");

        // The second line should fit within wrap_width and end with ellipsis.
        let second_line_width: Pixels = parts[1].chars().map(|c| wrapper.width_for_char(c)).sum();
        assert!(
            second_line_width <= wrap_width,
            "Second line '{}' ({}px) exceeds wrap_width ({}px)",
            parts[1],
            second_line_width,
            wrap_width
        );
        assert!(
            truncated.ends_with('\u{2026}'),
            "Should end with ellipsis: '{}'",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_newline_on_last_line() {
        let mut wrapper = build_wrapper();

        // "hello\nworld\nmore" with line_clamp(2):
        // Line 1: "hello", Line 2: "world" — but there's a third line,
        // so line 2 should be truncated with ellipsis.
        let text: &str = "hello\nworld\nmore";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        let parts: Vec<&str> = truncated.splitn(2, '\n').collect();
        assert_eq!(parts[0], "hello");
        assert!(
            truncated.ends_with('\u{2026}'),
            "Should end with ellipsis since there's more content: '{}'",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_trailing_newline() {
        let mut wrapper = build_wrapper();

        // "hello\nworld\n" with line_clamp(2):
        // The trailing newline has no content after it, so no ellipsis.
        let text: &str = "hello\nworld\n";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (result, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        assert!(
            !result.ends_with('\u{2026}'),
            "Trailing newline with no content should not add ellipsis: '{}'",
            result
        );
    }

    #[test]
    fn test_multiline_truncation_newline_fits_exactly() {
        let mut wrapper = build_wrapper();

        // "hello\nworld" with line_clamp(2):
        // Exactly 2 lines, no truncation needed.
        let text: &str = "hello\nworld";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (result, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        assert_eq!(
            result.as_ref(),
            text,
            "Text that fits exactly should not be modified: '{}'",
            result
        );
    }
}
