//! Splits markdown into the parts the preview renders differently: runs of ordinary markdown
//! (drawn by egui_commonmark) and tables, which the preview lays out itself because
//! egui_commonmark's tables never wrap and overflow narrow panes.
//!
//! egui_commonmark also mishandles a table nested in a quote (it prints the cells as loose
//! words and then garbles what follows), so a quote holding a table becomes a [`Block::Quote`]
//! of its own split contents. And inside list items it puts code blocks on the same line as
//! the item's text, squeezed into what's left of the row; a list holding a code block or a
//! table becomes a [`Block::List`] whose items are split (and laid out) the same way.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Markdown source to render as is.
    Text(String),
    Table(Table),
    /// A block quote (one that contains a table), with its contents split the same way.
    Quote(Vec<Block>),
    /// A list (one that contains a code block or a table), each item split the same way.
    List(List),
}

#[derive(Clone, Debug, PartialEq)]
pub struct List {
    /// Number of the first item, for an ordered list.
    pub start: Option<u64>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// `Some(checked)` for a task list item (`- [ ]` / `- [x]`).
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// Markdown source of each header cell.
    pub header: Vec<String>,
    /// Markdown source of each body cell, row by row.
    pub rows: Vec<Vec<String>>,
}

/// Same extensions egui_commonmark parses with, so both agree on what is a table.
fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_DEFINITION_LIST
}

pub fn split(source: &str) -> Vec<Block> {
    let parser = Parser::new_ext(source, options());
    // Reference-style link definitions (`[x]: https://…`) apply document-wide; every text run
    // gets a copy so links keep working after the split.
    let refdefs: String = parser
        .reference_definitions()
        .iter()
        .map(|(_, def)| format!("\n{}", source[def.span.clone()].trim_end()))
        .collect();
    split_with(source, &refdefs)
}

#[derive(Clone, Copy, PartialEq)]
enum Container {
    Quote,
    List,
    Other,
}

fn split_with(source: &str, refdefs: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut text_start = 0;
    // Open containers: kind, where it started, and whether it needs splitting (a table inside,
    // or for lists, a code block).
    let mut stack: Vec<(Container, usize, bool)> = Vec::new();
    let mut table: Option<(usize, Table)> = None;
    let mut in_head = false;
    let mut row: Vec<String> = Vec::new();

    for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote(_)) => stack.push((Container::Quote, range.start, false)),
            Event::Start(Tag::List(_)) => stack.push((Container::List, range.start, false)),
            Event::Start(Tag::FootnoteDefinition(_)) => {
                stack.push((Container::Other, range.start, false))
            }
            Event::End(TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::FootnoteDefinition) => {
                let Some((kind, start, has_table)) = stack.pop() else {
                    continue;
                };
                if !stack.is_empty() || !has_table {
                    continue;
                }
                match kind {
                    Container::List => {
                        push_text(&mut blocks, &source[text_start..start], refdefs);
                        blocks.push(Block::List(parse_list(&source[start..range.end], refdefs)));
                        text_start = range.end;
                    }
                    Container::Quote => {
                        push_text(&mut blocks, &source[text_start..start], refdefs);
                        let inner = unquote(&source[start..range.end]);
                        blocks.push(Block::Quote(split_with(&inner, refdefs)));
                        text_start = range.end;
                    }
                    // End the run here, so egui_commonmark starts fresh after it.
                    Container::Other => {
                        push_text(&mut blocks, &source[text_start..range.end], refdefs);
                        text_start = range.end;
                    }
                }
            }
            Event::Start(Tag::Table(_)) if stack.is_empty() => {
                table = Some((
                    range.start,
                    Table {
                        header: Vec::new(),
                        rows: Vec::new(),
                    },
                ))
            }
            Event::Start(Tag::Table(_)) => {
                if let Some(outer) = stack.first_mut() {
                    outer.2 = true;
                }
            }
            Event::Start(Tag::CodeBlock(_))
                if stack.iter().any(|(kind, ..)| *kind == Container::List) =>
            {
                if let Some(outer) = stack.first_mut() {
                    outer.2 = true;
                }
            }
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => {
                in_head = false;
                if let Some((_, t)) = &mut table {
                    t.header = std::mem::take(&mut row);
                }
            }
            Event::Start(Tag::TableCell) if table.is_some() => {
                row.push(source[range].trim().trim_matches('|').trim().to_string());
            }
            Event::End(TagEnd::TableRow) if !in_head => {
                if let Some((_, t)) = &mut table {
                    t.rows.push(std::mem::take(&mut row));
                }
            }
            Event::End(TagEnd::Table) => {
                if let Some((start, t)) = table.take() {
                    push_text(&mut blocks, &source[text_start..start], refdefs);
                    blocks.push(Block::Table(t));
                    text_start = range.end;
                }
            }
            _ => {}
        }
    }
    push_text(&mut blocks, &source[text_start..], refdefs);
    blocks
}

/// Split a list's source into its items, each item's content dedented to stand on its own
/// and split recursively.
fn parse_list(list: &str, refdefs: &str) -> List {
    let mut start = None;
    let mut items = Vec::new();
    let mut depth = 0usize;
    // The open top-level item: its byte range start and task state.
    let mut current: Option<(usize, Option<bool>)> = None;
    for (event, range) in Parser::new_ext(list, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::List(first)) => {
                if depth == 0 {
                    start = first;
                }
                depth += 1;
            }
            Event::End(TagEnd::List(_)) => depth = depth.saturating_sub(1),
            Event::Start(Tag::Item) if depth == 1 => current = Some((range.start, None)),
            Event::TaskListMarker(checked) if depth == 1 => {
                if let Some((_, task)) = &mut current {
                    *task = Some(checked);
                }
            }
            Event::End(TagEnd::Item) if depth == 1 => {
                if let Some((item_start, task)) = current.take() {
                    let mut content = item_content(&list[item_start..range.end]);
                    if task.is_some() {
                        content = strip_task_marker(&content);
                    }
                    items.push(Item {
                        task,
                        blocks: split_with(&content, refdefs),
                    });
                }
            }
            _ => {}
        }
    }
    List { start, items }
}

/// An item's markdown without its marker (`- `, `1. `), its other lines dedented to match.
fn item_content(item: &str) -> String {
    let first = item.lines().next().unwrap_or_default();
    let marker = first
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_digit() || matches!(c, '-' | '*' | '+' | '.' | ')')))
        .map_or(first.len(), |(i, _)| i);
    let spaces = first[marker..].chars().take_while(|c| *c == ' ').count();
    // Five or more spaces after the marker start an indented code block inside the item.
    let spaces = if spaces >= 5 || marker + spaces == first.len() {
        1
    } else {
        spaces
    };
    let content_col = marker + spaces;
    let mut out = String::with_capacity(item.len());
    for (i, line) in item.lines().enumerate() {
        if i == 0 {
            out.push_str(first.get(content_col..).unwrap_or_default());
        } else {
            out.push('\n');
            let indent = line.chars().take_while(|c| *c == ' ').count();
            out.push_str(&line[indent.min(content_col)..]);
        }
    }
    out
}

/// Remove the `[ ]` / `[x]` that starts a task item's text.
fn strip_task_marker(content: &str) -> String {
    let t = content.trim_start();
    match t.get(..3) {
        Some("[ ]" | "[x]" | "[X]") => t[3..].trim_start().to_string(),
        _ => content.to_string(),
    }
}

/// The contents of a block quote: one level of `>` (and the space after it) removed from
/// each line. Lazy continuation lines without a `>` are kept as they are.
fn unquote(quote: &str) -> String {
    quote
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            match trimmed.strip_prefix('>') {
                Some(rest) if line.len() - trimmed.len() <= 3 => {
                    rest.strip_prefix(' ').unwrap_or(rest)
                }
                _ => line,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn push_text(blocks: &mut Vec<Block>, text: &str, refdefs: &str) {
    if !text.trim().is_empty() {
        blocks.push(Block::Text(format!("{text}{refdefs}")));
    }
}

/// Column widths for a table: natural widths when they fit in `available`, otherwise columns
/// narrower than a fair share keep their width and the rest split what remains (so short
/// columns stay unwrapped and long ones wrap).
pub fn fit_columns(natural: &[f32], available: f32) -> Vec<f32> {
    if natural.iter().sum::<f32>() <= available {
        return natural.to_vec();
    }
    let mut widths = vec![0.0; natural.len()];
    let mut open: Vec<usize> = (0..natural.len()).collect();
    let mut remaining = available;
    while !open.is_empty() {
        let share = remaining / open.len() as f32;
        let (fits, wide): (Vec<usize>, Vec<usize>) =
            open.iter().partition(|&&i| natural[i] <= share);
        if fits.is_empty() {
            for i in wide {
                widths[i] = share;
            }
            break;
        }
        for i in fits {
            widths[i] = natural[i];
            remaining -= natural[i];
        }
        open = wide;
    }
    widths
}

/// The visible text of inline markdown, roughly: for measuring how wide a cell wants to be.
pub fn plain_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '`' | '*' | '_' | '~' | '\\' => {}
            // [text](url): keep the text, drop the url.
            ']' if chars.peek() == Some(&'(') => {
                for c in chars.by_ref() {
                    if c == ')' {
                        break;
                    }
                }
            }
            '[' | ']' | '!' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_columns() {
        // Everything fits: natural widths.
        assert_eq!(fit_columns(&[50.0, 80.0], 200.0), [50.0, 80.0]);
        // A short column keeps its width; the long ones share the rest.
        assert_eq!(
            fit_columns(&[40.0, 500.0, 300.0], 340.0),
            [40.0, 150.0, 150.0]
        );
        // All wide: equal split.
        assert_eq!(fit_columns(&[400.0, 400.0], 300.0), [150.0, 150.0]);
    }

    #[test]
    fn plain_text_for_measuring() {
        assert_eq!(plain_text("`Alt+P` (also **this**)"), "Alt+P (also this)");
        assert_eq!(plain_text("[egui](https://x.y/z) rocks"), "egui rocks");
    }

    #[test]
    fn splits_out_top_level_tables() {
        let md = "# Title\n\nIntro.\n\n| Key | Action |\n|---|---|\n| `Alt+P` | Open the **palette** |\n| `Ctrl+Tab` | Next |\n\nAfter.\n";
        let blocks = split(md);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Text(t) if t.contains("Intro.") && !t.contains('|')));
        let Block::Table(t) = &blocks[1] else {
            panic!("expected a table: {blocks:?}");
        };
        assert_eq!(t.header, ["Key", "Action"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], ["`Alt+P`", "Open the **palette**"]);
        assert!(matches!(&blocks[2], Block::Text(t) if t.trim() == "After."));
    }

    #[test]
    fn quotes_with_tables_are_split_inside() {
        let md = "> Note:\n>\n> | a | b |\n> |---|---|\n> | 1 | 2 |\n\n![img](x.png)\n";
        let blocks = split(md);
        assert_eq!(blocks.len(), 2, "{blocks:?}");
        let Block::Quote(inner) = &blocks[0] else {
            panic!("expected a quote: {blocks:?}");
        };
        assert!(matches!(&inner[0], Block::Text(t) if t.trim() == "Note:"));
        assert!(matches!(&inner[1], Block::Table(t) if t.rows == [["1", "2"]]));
        // What follows renders in a fresh run.
        assert!(matches!(&blocks[1], Block::Text(t) if t.trim() == "![img](x.png)"));
    }

    #[test]
    fn plain_quotes_and_text_stay_whole() {
        let md = "> just a quote\n\ntext\n";
        assert_eq!(split(md), [Block::Text(md.to_string())]);
        assert_eq!(split("just text"), [Block::Text("just text".into())]);
        assert!(split("").is_empty());
    }

    #[test]
    fn lists_with_code_blocks_are_split_into_items() {
        let md = "Intro\n\n1. Plain item\n2. Item with code:\n   ```toml\n   a = 1\n   ```\n   More text.\n   - nested\n3. Last\n\nafter\n";
        let blocks = split(md);
        assert_eq!(blocks.len(), 3, "{blocks:?}");
        let Block::List(list) = &blocks[1] else {
            panic!("expected a list: {blocks:?}");
        };
        assert_eq!(list.start, Some(1));
        assert_eq!(list.items.len(), 3);
        let Block::Text(second) = &list.items[1].blocks[0] else {
            panic!("expected text");
        };
        // The code block is dedented to stand on its own, so it renders as a block.
        assert!(second.starts_with("Item with code:\n```toml\na = 1\n```\nMore text.\n- nested"));
        assert!(matches!(&blocks[2], Block::Text(t) if t.trim() == "after"));
    }

    #[test]
    fn list_items_keep_task_state_and_tables() {
        let md = "- [x] done\n- [ ] todo\n\n  | a |\n  |---|\n  | 1 |\n";
        let blocks = split(md);
        let Block::List(list) = &blocks[0] else {
            panic!("expected a list: {blocks:?}");
        };
        assert_eq!(list.start, None);
        assert_eq!(list.items[0].task, Some(true));
        assert!(matches!(&list.items[0].blocks[0], Block::Text(t) if t.trim() == "done"));
        assert_eq!(list.items[1].task, Some(false));
        assert!(matches!(&list.items[1].blocks[1], Block::Table(_)));
    }

    #[test]
    fn plain_lists_stay_with_commonmark() {
        let md = "- a\n- b\n  - c\n";
        assert_eq!(split(md), [Block::Text(md.to_string())]);
    }

    #[test]
    fn keeps_reference_links_working_in_every_run() {
        let md = "See [docs][d].\n\n| a |\n|---|\n| 1 |\n\nAgain [docs][d].\n\n[d]: https://example.com\n";
        let blocks = split(md);
        let texts: Vec<&String> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 2);
        assert!(texts.iter().all(|t| t.contains("[d]: https://example.com")));
    }
}
