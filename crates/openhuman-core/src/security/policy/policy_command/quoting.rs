//! Quote-aware shell-string scanning: splitting a command into `;`/`|`/`&&`/
//! `||`-separated segments, and detecting unquoted control characters
//! (`&`, arbitrary redirects) without being fooled by quoted literals, plus a
//! pre-pass that blanks quoted-heredoc bodies so document text is not scanned
//! as shell syntax.

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    None,
    Single,
    Double,
}

/// Split a shell command into sub-commands by unquoted separators.
///
/// Separators:
/// - `;` and newline
/// - `|`
/// - `&&`, `||`
///
/// Characters inside single or double quotes are treated as literals, so
/// `sqlite3 db "SELECT 1; SELECT 2;"` remains a single segment.
pub(in crate::security::policy) fn split_unquoted_segments(command: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut quote = QuoteState::None;
    let mut escaped = false;
    let mut chars = command.chars().peekable();

    let push_segment = |segments: &mut Vec<String>, current: &mut String| {
        let trimmed = current.trim();
        if !trimmed.is_empty() {
            segments.push(trimmed.to_string());
        }
        current.clear();
    };

    while let Some(ch) = chars.next() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
                current.push(ch);
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    current.push(ch);
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    current.push(ch);
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                }
                current.push(ch);
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    current.push(ch);
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    current.push(ch);
                    continue;
                }

                match ch {
                    '\'' => {
                        quote = QuoteState::Single;
                        current.push(ch);
                    }
                    '"' => {
                        quote = QuoteState::Double;
                        current.push(ch);
                    }
                    ';' | '\n' => push_segment(&mut segments, &mut current),
                    '|' => {
                        if chars.next_if_eq(&'|').is_some() {
                            // Consume full `||`; both characters are separators.
                        }
                        push_segment(&mut segments, &mut current);
                    }
                    '&' => {
                        if chars.next_if_eq(&'&').is_some() {
                            // `&&` is a separator; single `&` is handled separately.
                            push_segment(&mut segments, &mut current);
                        } else {
                            current.push(ch);
                        }
                    }
                    _ => current.push(ch),
                }
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    segments
}

/// Detect a single unquoted `&` operator (background/chain). `&&` is allowed.
///
/// We treat any standalone `&` as unsafe in policy validation because it can
/// chain hidden sub-commands and escape foreground timeout expectations.
pub(in crate::security::policy) fn contains_unquoted_single_ampersand(command: &str) -> bool {
    let mut quote = QuoteState::None;
    let mut escaped = false;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                match ch {
                    '\'' => quote = QuoteState::Single,
                    '"' => quote = QuoteState::Double,
                    '&' if chars.next_if_eq(&'&').is_none() => {
                        return true;
                    }
                    _ => {}
                }
            }
        }
    }

    false
}

/// Like [`contains_unquoted_single_ampersand`] but ignores file-descriptor
/// duplication redirects, where the `&` is part of a redirect operator rather
/// than a background/separator: `2>&1`, `>&2` (prev char `>`), and `&>file`
/// (next char `>`). Used by [`super::classification::has_hidden_execution`] so
/// a benign `… 2>&1` — which `classify_command` already accounts for as a
/// `Write` redirect — is not mistaken for a backgrounded command and
/// hard-blocked after the human approved it. A standalone `&` (e.g. `cmd &`,
/// `a & b`) still returns true, since it can run a second command
/// `classify_command` wouldn't see.
pub(super) fn contains_unquoted_background_ampersand(command: &str) -> bool {
    let mut quote = QuoteState::None;
    let mut escaped = false;
    let mut prev = '\0';
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    prev = ch;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    prev = ch;
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    prev = ch;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    prev = ch;
                    continue;
                }
                match ch {
                    '\'' => quote = QuoteState::Single,
                    '"' => quote = QuoteState::Double,
                    '&' => {
                        if chars.next_if_eq(&'&').is_some() {
                            // `&&` logical AND — consume both, not background.
                        } else {
                            let next = chars.peek().copied().unwrap_or('\0');
                            // Skip fd-dup redirects: `2>&1`/`>&2` (prev `>`) and
                            // `&>file` (next `>`).
                            if prev != '>' && next != '>' {
                                return true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        prev = ch;
    }

    false
}

/// Detect an unquoted character in a shell command.
pub(in crate::security::policy) fn contains_unquoted_char(command: &str, target: char) -> bool {
    let mut quote = QuoteState::None;
    let mut escaped = false;

    for ch in command.chars() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                    continue;
                }
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                match ch {
                    '\'' => quote = QuoteState::Single,
                    '"' => quote = QuoteState::Double,
                    _ if ch == target => return true,
                    _ => {}
                }
            }
        }
    }

    false
}

/// Blank out the body of every **quoted-delimiter** heredoc in `command`.
///
/// `classify_command` and the structural guards scan the raw command string, so
/// until now a heredoc body was read as live shell text. That is wrong for a
/// quoted delimiter: in `cat > f << 'EOF' … EOF` the shell performs no
/// expansion inside the body, so an `&`, a `` ` `` or a `$(` there is data.
/// Measured consequence (life-scenario `meal-plan`): a recipe document was
/// refused as "background (&) is not allowed" because four dinner titles read
/// "Chicken & Spinach".
///
/// An **unquoted** delimiter (`<< EOF`) is deliberately left alone — expansion
/// *is* live inside that body, so `$(rm -rf ~)` there really would execute and
/// must keep tripping the guard.
///
/// Body lines are replaced by empty lines rather than removed, so line counts,
/// the surrounding command text and the `>` of the redirect are all preserved:
/// a heredoc write still classifies as [`CommandClass::Write`] and still
/// prompts. Returns [`Cow::Borrowed`] when the command has no quoted heredoc,
/// which is the overwhelmingly common case.
pub(in crate::security::policy) fn strip_quoted_heredoc_bodies(command: &str) -> Cow<'_, str> {
    if !command.contains("<<") {
        return Cow::Borrowed(command);
    }

    let mut delimiters: Vec<String> = Vec::new();
    let mut quote = QuoteState::None;
    let mut escaped = false;
    let mut chars = command.char_indices().peekable();
    // Byte offsets of the `<<` operators whose delimiter is quoted, in order.
    let mut operators: Vec<usize> = Vec::new();

    while let Some((idx, ch)) = chars.next() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '"' => quote = QuoteState::None,
                    _ => {}
                }
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '\'' => quote = QuoteState::Single,
                    '"' => quote = QuoteState::Double,
                    '<' if chars.next_if(|(_, c)| *c == '<').is_some() => {
                        // `<<<` is a here-string, not a heredoc: no body follows.
                        if chars.next_if(|(_, c)| *c == '<').is_some() {
                            continue;
                        }
                        let rest = &command[idx + 2..];
                        if let Some((delim, consumed)) = quoted_heredoc_delimiter(rest) {
                            delimiters.push(delim);
                            operators.push(idx);
                            // Skip past the delimiter token so its quotes do not
                            // re-enter the quote state machine.
                            for _ in 0..consumed {
                                chars.next();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if operators.is_empty() {
        return Cow::Borrowed(command);
    }

    // Walk the lines. Once the line carrying the Nth operator ends, every
    // following line is body until its terminator line appears.
    let mut out = String::with_capacity(command.len());
    let mut next_delim = 0usize;
    let mut open: Option<String> = None;
    let mut consumed_bytes = 0usize;

    for line in command.split_inclusive('\n') {
        let line_start = consumed_bytes;
        consumed_bytes += line.len();

        if let Some(delim) = open.clone() {
            if line.trim() == delim {
                open = None;
                out.push_str(line);
            } else {
                // Blank the body, keeping the newline so offsets stay sane.
                if line.ends_with('\n') {
                    out.push('\n');
                }
            }
            continue;
        }

        out.push_str(line);
        let line_end = line_start + line.len();
        while next_delim < operators.len() && operators[next_delim] < line_end {
            // The last operator on a line wins: `cat << 'A' << 'B'` reads A's
            // body first, but only tracking one at a time is enough for the
            // guard, and a nested case simply keeps scanning as today.
            open = Some(delimiters[next_delim].clone());
            next_delim += 1;
        }
    }

    Cow::Owned(out)
}

/// Parse a heredoc delimiter token immediately after `<<`, returning the
/// delimiter and how many chars of `rest` it spans — **only** when the token is
/// quoted (`'EOF'` or `"EOF"`), optionally preceded by `-` and whitespace.
/// An unquoted delimiter returns `None`, because its body is still expanded.
fn quoted_heredoc_delimiter(rest: &str) -> Option<(String, usize)> {
    let mut consumed = 0usize;
    let mut chars = rest.chars().peekable();

    if chars.peek() == Some(&'-') {
        chars.next();
        consumed += 1;
    }
    while chars.peek().is_some_and(|c| *c == ' ' || *c == '\t') {
        chars.next();
        consumed += 1;
    }

    let quote = match chars.next() {
        Some(q @ ('\'' | '"')) => q,
        _ => return None,
    };
    consumed += 1;

    let mut delim = String::new();
    for c in chars {
        consumed += 1;
        if c == quote {
            return (!delim.is_empty()).then_some((delim, consumed));
        }
        if c == '\n' {
            return None;
        }
        delim.push(c);
    }
    None
}
