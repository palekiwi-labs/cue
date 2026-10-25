//! In-place edits of a markdown artifact's frontmatter block.
//!
//! Edits work on the text of the block, never on a parsed and reserialized
//! form, so every byte outside the edited line is preserved: key order,
//! quoting, comments and the body are left exactly as written.
//!
//! Only top-level keys holding a single-line scalar can be edited. A key
//! whose value continues onto further lines is refused rather than risk
//! leaving part of its value behind.

use anyhow::{Result, bail};

/// Append `key: value` as the last frontmatter line when `key` is absent.
///
/// Returns the edited text, or `None` when `key` is already present, in which
/// case its existing value is kept.
pub fn insert_if_absent(text: &str, key: &str, value: &str) -> Result<Option<String>> {
    if value.contains(['\n', '\r']) {
        bail!("frontmatter value for '{key}' must be a single line");
    }
    let block = Block::parse(text)?;
    if block.find(key)?.is_some() {
        return Ok(None);
    }
    let at = block.closing_start;
    let line = format!("{key}: {value}{}", block.newline);
    Ok(Some(format!("{}{line}{}", &text[..at], &text[at..])))
}

/// Remove the line holding `key`.
///
/// Returns the edited text, or `None` when `key` is absent.
pub fn remove(text: &str, key: &str) -> Result<Option<String>> {
    let block = Block::parse(text)?;
    let Some(line) = block.find(key)? else {
        return Ok(None);
    };
    Ok(Some(format!(
        "{}{}",
        &text[..line.start],
        &text[line.end..]
    )))
}

/// The frontmatter lines of a document, as byte ranges into it.
struct Block<'a> {
    lines: Vec<Line<'a>>,
    /// Byte offset of the closing fence line.
    closing_start: usize,
    /// Line ending of the opening fence, reused for inserted lines.
    newline: &'static str,
}

struct Line<'a> {
    start: usize,
    end: usize,
    /// Content without its line ending.
    content: &'a str,
}

impl<'a> Block<'a> {
    fn parse(text: &'a str) -> Result<Self> {
        let mut offset = 0;
        let mut raw = text.split_inclusive('\n').map(|line| {
            let start = offset;
            offset += line.len();
            Line {
                start,
                end: offset,
                content: line.trim_end_matches(['\n', '\r']),
            }
        });

        let opening = raw.next().filter(|line| line.content == "---");
        let Some(opening) = opening else {
            bail!("document has no frontmatter");
        };
        let newline = if text[..opening.end].ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };

        let mut lines = Vec::new();
        for line in raw {
            if line.content == "---" {
                return Ok(Self {
                    lines,
                    closing_start: line.start,
                    newline,
                });
            }
            lines.push(line);
        }
        bail!("frontmatter is not terminated")
    }

    /// The line holding top-level `key`, refusing a value that spans lines.
    fn find(&self, key: &str) -> Result<Option<&Line<'a>>> {
        let Some(index) = self.lines.iter().position(|line| {
            line.content
                .strip_prefix(key)
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
        }) else {
            return Ok(None);
        };

        let line = &self.lines[index];
        let value = line.content[key.len() + 1..].trim();
        let continued = self.lines.get(index + 1).is_some_and(|next| {
            next.content.starts_with([' ', '\t']) || next.content.starts_with('-')
        });
        if value.is_empty() || value.starts_with(['|', '>']) || continued {
            bail!("frontmatter key '{key}' does not hold a single-line value");
        }
        Ok(Some(line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "---\n\
title: 'Quoted: title'\n\
kind: work\n\
refs:\n\
- a/b/c\n\
created_at: 1792913657 # stamped\n\
---\n\
Body with\n---\nfence-like lines.\n";

    #[test]
    fn insert_appends_last_and_preserves_every_other_byte() {
        let edited = insert_if_absent(DOC, "archived_at", "1800000000")
            .unwrap()
            .unwrap();
        let expected = DOC.replacen(
            "# stamped\n---\n",
            "# stamped\narchived_at: 1800000000\n---\n",
            1,
        );
        assert_eq!(edited, expected);
    }

    #[test]
    fn insert_keeps_an_existing_value() {
        let archived = DOC.replacen("kind: work\n", "kind: work\narchived_at: 1\n", 1);
        assert_eq!(
            insert_if_absent(&archived, "archived_at", "2").unwrap(),
            None
        );
    }

    #[test]
    fn insert_matches_whole_top_level_keys_only() {
        let doc = DOC.replacen(
            "kind: work\n",
            "kind: work\narchived_at_note: x\nmeta:\n  archived_at: 1\n",
            1,
        );
        let edited = insert_if_absent(&doc, "archived_at", "2").unwrap().unwrap();
        assert!(edited.contains("# stamped\narchived_at: 2\n---\n"));
    }

    #[test]
    fn remove_deletes_only_that_line() {
        let archived = DOC.replacen("kind: work\n", "kind: work\narchived_at: 1\n", 1);
        assert_eq!(remove(&archived, "archived_at").unwrap().unwrap(), DOC);
    }

    #[test]
    fn remove_of_an_absent_key_is_unchanged() {
        assert_eq!(remove(DOC, "archived_at").unwrap(), None);
    }

    #[test]
    fn remove_ignores_body_lines() {
        let doc = format!("{DOC}archived_at: 1\n");
        assert_eq!(remove(&doc, "archived_at").unwrap(), None);
    }

    #[test]
    fn multi_line_values_are_refused() {
        assert!(remove(DOC, "refs").is_err());
        let block = DOC.replacen("kind: work\n", "kind: work\narchived_at: >\n  1\n", 1);
        assert!(insert_if_absent(&block, "archived_at", "2").is_err());
    }

    #[test]
    fn values_spanning_lines_are_refused() {
        assert!(insert_if_absent(DOC, "archived_at", "1\nkind: x").is_err());
    }

    #[test]
    fn missing_or_unterminated_frontmatter_is_an_error() {
        assert!(insert_if_absent("no fence\n", "k", "v").is_err());
        assert!(insert_if_absent("---\nkind: work\n", "k", "v").is_err());
        assert!(remove("---\nkind: work\n", "k").is_err());
    }

    #[test]
    fn crlf_files_keep_their_line_endings() {
        let doc = "---\r\nkind: work\r\n---\r\nbody\r\n";
        let edited = insert_if_absent(doc, "archived_at", "1").unwrap().unwrap();
        assert_eq!(
            edited,
            "---\r\nkind: work\r\narchived_at: 1\r\n---\r\nbody\r\n"
        );
        assert_eq!(remove(&edited, "archived_at").unwrap().unwrap(), doc);
    }
}
