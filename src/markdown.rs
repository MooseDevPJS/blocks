//! Analisi del Markdown per l'anteprima live.
//!
//! Il testo viene diviso in righe; per ogni riga si riconosce il tipo di blocco
//! (titolo, elenco, checklist…) e gli stili in linea (grassetto, link, tag…).
//! Le posizioni sono in **byte** rispetto all'inizio del documento.
//! Il parser non modifica mai il testo: serve solo a decidere come disegnarlo.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// Prima riga non vuota della nota: il titolo.
    Title,
    Heading(u8),
    Paragraph,
    Bullet,
    Ordered,
    Task {
        checked: bool,
    },
    Quote,
    Rule,
    CodeFence,
    Code,
    /// Riga che contiene solo un'immagine: `![testo](percorso)`.
    Image,
}

/// Come appare un pezzo di testo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fmt {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: bool,
    pub tag: bool,
    /// Sintassi (`**`, `[`, `](url)`…): nascosta fuori dalla riga del cursore.
    pub marker: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Url(String),
    Note(String),
    Tag(String),
}

#[derive(Clone, Debug)]
pub struct Span {
    pub range: Range<usize>,
    pub fmt: Fmt,
    pub target: Option<Target>,
}

#[derive(Clone, Debug)]
pub struct Line {
    /// Righe senza il `\n` finale.
    pub range: Range<usize>,
    pub kind: LineKind,
    /// Prefisso del blocco (`## `, `- [ ] `, `> `…), vuoto se assente.
    pub prefix: Range<usize>,
    /// Stili in linea dopo il prefisso; coprono tutto il resto della riga.
    pub spans: Vec<Span>,
    /// Per le righe immagine: il percorso dell'immagine.
    pub src: Option<String>,
}

pub fn parse(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut in_code = false;
    let mut seen_title = false;
    for raw in text.split('\n') {
        let range = start..start + raw.len();
        start = range.end + 1;
        let (kind, prefix_len) = if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            (LineKind::CodeFence, raw.len())
        } else if in_code {
            (LineKind::Code, 0)
        } else {
            block_kind(raw)
        };
        let kind = if !seen_title && !raw.trim().is_empty() && !matches!(kind, LineKind::CodeFence | LineKind::Code | LineKind::Image) {
            seen_title = true;
            LineKind::Title
        } else {
            kind
        };
        if kind == LineKind::Title {
            seen_title = true;
        }
        let prefix_len = if kind == LineKind::Title { heading_prefix(raw).unwrap_or((0, 0)).1 } else { prefix_len };
        let prefix = range.start..range.start + prefix_len;
        let mut spans = Vec::new();
        if matches!(kind, LineKind::Code | LineKind::CodeFence) {
            push_plain(&mut spans, prefix.end..range.end, Fmt { code: true, ..Fmt::default() });
        } else {
            let base = Fmt { strike: matches!(kind, LineKind::Task { checked: true }), ..Fmt::default() };
            parse_inline(text, prefix.end..range.end, base, &mut spans);
        }
        let src = (kind == LineKind::Image).then(|| image_src(raw)).flatten();
        lines.push(Line { range, kind, prefix, spans, src });
    }
    lines
}

fn heading_prefix(line: &str) -> Option<(u8, usize)> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        let spaces = line[hashes..].bytes().take_while(|&b| b == b' ').count();
        Some((hashes as u8, hashes + spaces))
    } else {
        None
    }
}

/// Tipo di blocco e lunghezza in byte del suo prefisso.
fn block_kind(line: &str) -> (LineKind, usize) {
    if image_src(line).is_some() {
        return (LineKind::Image, line.len());
    }
    if let Some((level, len)) = heading_prefix(line) {
        return (LineKind::Heading(level), len);
    }
    let trimmed = line.trim_end();
    if trimmed.len() >= 3 && (trimmed.bytes().all(|b| b == b'-') || trimmed.bytes().all(|b| b == b'*')) {
        return (LineKind::Rule, line.len());
    }
    let indent = line.bytes().take_while(|&b| b == b' ' || b == b'\t').count();
    let rest = &line[indent..];
    if rest.starts_with("> ") || rest == ">" {
        return (LineKind::Quote, indent + rest.len().min(2));
    }
    for (marker, checked) in [("- [ ] ", false), ("- [x] ", true), ("- [X] ", true), ("* [ ] ", false), ("* [x] ", true)] {
        if rest.starts_with(marker) {
            return (LineKind::Task { checked }, indent + marker.len());
        }
    }
    if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        return (LineKind::Bullet, indent + 2);
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=9).contains(&digits) && rest[digits..].starts_with(". ") {
        return (LineKind::Ordered, indent + digits + 2);
    }
    (LineKind::Paragraph, 0)
}

/// Per gli elenchi: tipo di riga, lunghezza del prefisso e prefisso da usare
/// per la riga successiva quando si preme Invio.
pub fn list_continuation(line: &str) -> Option<(LineKind, usize, String)> {
    let (kind, len) = block_kind(line);
    let indent = &line[..line.len() - line.trim_start().len()];
    let next = match kind {
        LineKind::Bullet => line[..len].to_owned(),
        LineKind::Task { .. } => format!("{indent}{}", if line.trim_start().starts_with('*') { "* [ ] " } else { "- [ ] " }),
        LineKind::Quote => line[..len].to_owned(),
        LineKind::Ordered => {
            let rest = &line[indent.len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            format!("{indent}{}. ", digits.parse::<u64>().unwrap_or(0) + 1)
        }
        _ => return None,
    };
    Some((kind, len, next))
}

/// `![alt](src)` da solo sulla riga.
fn image_src(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("![")?;
    let mid = rest.find("](")?;
    let src = rest[mid + 2..].strip_suffix(')')?;
    (!src.is_empty() && !src.contains(')')).then(|| src.trim().to_owned())
}

fn push_plain(out: &mut Vec<Span>, range: Range<usize>, fmt: Fmt) {
    if !range.is_empty() {
        out.push(Span { range, fmt, target: None });
    }
}

fn marker(out: &mut Vec<Span>, range: Range<usize>, fmt: Fmt) {
    push_plain(out, range, Fmt { marker: true, ..fmt });
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// Stili in linea su `text[range]`. I pezzi prodotti coprono tutto `range`.
fn parse_inline(text: &str, range: Range<usize>, fmt: Fmt, out: &mut Vec<Span>) {
    let s = &text[range.clone()];
    let at = |i: usize| range.start + i;
    let mut plain_from = 0;
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let prev = s[..i].chars().next_back();
        let mut matched: Option<usize> = None; // fine (relativa) del costrutto riconosciuto

        if let Some(after) = rest.strip_prefix('`') {
            if let Some(end) = after.find('`') {
                let close = i + 1 + end;
                push_plain(out, at(plain_from)..at(i), fmt);
                marker(out, at(i)..at(i + 1), fmt);
                push_plain(out, at(i + 1)..at(close), Fmt { code: true, ..fmt });
                marker(out, at(close)..at(close + 1), fmt);
                matched = Some(close + 1);
            }
        } else if let Some(after) = rest.strip_prefix("[[") {
            if let Some(end) = after.find("]]").filter(|&e| e > 0 && !after[..e].contains('\n')) {
                let inner = &rest[2..2 + end];
                let close = i + 2 + end;
                let (target, shown_from) = match inner.find('|') {
                    Some(p) => (&inner[..p], i + 2 + p + 1),
                    None => (inner, i + 2),
                };
                push_plain(out, at(plain_from)..at(i), fmt);
                marker(out, at(i)..at(shown_from), fmt);
                out.push(Span {
                    range: at(shown_from)..at(close),
                    fmt: Fmt { link: true, ..fmt },
                    target: Some(Target::Note(target.trim().to_owned())),
                });
                marker(out, at(close)..at(close + 2), fmt);
                matched = Some(close + 2);
            }
        } else if rest.starts_with('[') {
            // [testo](url)
            if let Some(mid) = rest.find("](")
                && let Some(end) = rest[mid + 2..].find(')')
            {
                let label = i + 1..i + mid;
                let url = &rest[mid + 2..mid + 2 + end];
                if !label.is_empty() && !url.is_empty() && !url.contains(' ') {
                    let close = i + mid + 2 + end + 1;
                    push_plain(out, at(plain_from)..at(i), fmt);
                    marker(out, at(i)..at(i + 1), fmt);
                    out.push(Span {
                        range: at(label.start)..at(label.end),
                        fmt: Fmt { link: true, ..fmt },
                        target: Some(Target::Url(url.to_owned())),
                    });
                    marker(out, at(label.end)..at(close), fmt);
                    matched = Some(close);
                }
            }
        } else if rest.starts_with("**") || rest.starts_with("__") || rest.starts_with("~~") {
            let delim = &rest[..2];
            if let Some(end) = rest[2..].find(delim).filter(|&e| e > 0) {
                let close = i + 2 + end;
                let inner_fmt = match delim {
                    "~~" => Fmt { strike: true, ..fmt },
                    _ => Fmt { bold: true, ..fmt },
                };
                push_plain(out, at(plain_from)..at(i), fmt);
                marker(out, at(i)..at(i + 2), fmt);
                parse_inline(text, at(i + 2)..at(close), inner_fmt, out);
                marker(out, at(close)..at(close + 2), fmt);
                matched = Some(close + 2);
            }
        } else if (rest.starts_with('*') || rest.starts_with('_')) && rest[1..].starts_with(|c: char| !c.is_whitespace()) {
            let d = &rest[..1];
            let word_start = prev.is_none_or(|p| !p.is_alphanumeric());
            if d == "*" || word_start {
                let close =
                    rest[1..].match_indices(d).map(|(e, _)| i + 1 + e).find(|&c| c > i + 1 && !s[..c].ends_with(char::is_whitespace));
                if let Some(close) = close {
                    push_plain(out, at(plain_from)..at(i), fmt);
                    marker(out, at(i)..at(i + 1), fmt);
                    parse_inline(text, at(i + 1)..at(close), Fmt { italic: true, ..fmt }, out);
                    marker(out, at(close)..at(close + 1), fmt);
                    matched = Some(close + 1);
                }
            }
        } else if rest.starts_with('#') && prev.is_none_or(char::is_whitespace) {
            let len: usize = rest[1..].chars().take_while(|&c| is_tag_char(c)).map(char::len_utf8).sum();
            let name = &rest[1..1 + len];
            if !name.is_empty() && name.chars().any(|c| !c.is_ascii_digit()) {
                push_plain(out, at(plain_from)..at(i), fmt);
                out.push(Span { range: at(i)..at(i + 1 + len), fmt: Fmt { tag: true, ..fmt }, target: Some(Target::Tag(name.to_owned())) });
                matched = Some(i + 1 + len);
            }
        } else if (rest.starts_with("https://") || rest.starts_with("http://")) && prev.is_none_or(|p| !p.is_alphanumeric()) {
            let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let url = rest[..len].trim_end_matches(['.', ',', ';', ':', ')', '!', '?']);
            push_plain(out, at(plain_from)..at(i), fmt);
            out.push(Span { range: at(i)..at(i + url.len()), fmt: Fmt { link: true, ..fmt }, target: Some(Target::Url(url.to_owned())) });
            matched = Some(i + url.len());
        }

        match matched {
            Some(end) => {
                i = end;
                plain_from = end;
            }
            None => i += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    push_plain(out, at(plain_from)..range.end, fmt);
}

/// Tutti i `#tag` di una nota (senza `#`, in minuscolo, senza doppioni).
pub fn tags(text: &str) -> Vec<String> {
    let mut out: Vec<String> = parse(text)
        .into_iter()
        .flat_map(|l| l.spans)
        .filter_map(|s| match s.target {
            Some(Target::Tag(t)) => Some(t.to_lowercase()),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Nomi delle note collegate con `[[ ]]` (in minuscolo, senza doppioni).
pub fn links(text: &str) -> Vec<String> {
    let mut out: Vec<String> = parse(text)
        .into_iter()
        .flat_map(|l| l.spans)
        .filter_map(|s| match s.target {
            Some(Target::Note(t)) => Some(t.to_lowercase()),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Sostituisce i link `[[vecchio]]` e `[[vecchio|alias]]` con il nuovo nome.
/// Restituisce `None` se nel testo non c'era nessun link da cambiare.
pub fn rename_links(text: &str, old: &str, new: &str) -> Option<String> {
    let old = old.trim().to_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut changed = false;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start + 2]);
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]").filter(|&e| !rest[..e].contains('\n')) else { continue };
        let inner = &rest[..end];
        let (target, alias) = match inner.find('|') {
            Some(p) => (&inner[..p], Some(&inner[p..])),
            None => (inner, None),
        };
        if target.trim().to_lowercase() == old {
            out.push_str(new);
            out.push_str(alias.unwrap_or(""));
            changed = true;
        } else {
            out.push_str(inner);
        }
        out.push_str("]]");
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(text: &str, line: &Line) -> String {
        line.spans.iter().filter(|s| !s.fmt.marker).map(|s| &text[s.range.clone()]).collect()
    }

    #[test]
    fn spans_cover_every_byte() {
        let text = "Titolo\n## Sotto **gras *cor*** e `code`\n- [ ] [link](https://x.it) [[Nota|alias]] #tag http://a.b.\n```\nlet x = 1;\n```\n> cit _sì_";
        for line in parse(text) {
            let mut pos = line.prefix.end;
            for s in &line.spans {
                assert_eq!(s.range.start, pos, "buco in {:?}", &text[line.range.clone()]);
                pos = s.range.end;
            }
            assert_eq!(pos, line.range.end);
        }
    }

    #[test]
    fn block_kinds() {
        let kinds: Vec<_> = parse("Titolo\n# H1\n- a\n1. b\n- [x] c\n> d\n---\n```\ncodice\n```\ntesto").iter().map(|l| l.kind).collect();
        use LineKind::*;
        assert_eq!(kinds, [Title, Heading(1), Bullet, Ordered, Task { checked: true }, Quote, Rule, CodeFence, Code, CodeFence, Paragraph]);
    }

    #[test]
    fn markers_are_hidden_from_shown_text() {
        let text = "T\nCiao **mondo** [sito](https://a.it) [[Nota|alias]] `x`";
        let lines = parse(text);
        assert_eq!(shown(text, &lines[1]), "Ciao mondo sito alias x");
        let targets: Vec<_> = lines[1].spans.iter().filter_map(|s| s.target.clone()).collect();
        assert_eq!(targets, [Target::Url("https://a.it".into()), Target::Note("Nota".into())]);
    }

    #[test]
    fn tags_are_found_but_not_headings_or_numbers() {
        assert_eq!(tags("# Titolo\nvedi #Casa e #lavoro/clienti, non #123 né a#b"), ["casa", "lavoro/clienti"]);
    }

    #[test]
    fn unicode_is_safe() {
        let text = "Città\n**perché** _è_ #caffè 🍕 [ok](https://x.it/à)";
        let lines = parse(text);
        assert_eq!(shown(text, &lines[1]), "perché è #caffè 🍕 ok");
    }

    #[test]
    fn images_and_links() {
        let lines = parse(
            "T
![foto](../_allegati/a b.png)
vedi ![x](y) qui",
        );
        assert_eq!(lines[1].kind, LineKind::Image);
        assert_eq!(lines[1].src.as_deref(), Some("../_allegati/a b.png"));
        assert_eq!(lines[2].kind, LineKind::Paragraph);
        assert_eq!(links("[[Uno]] e [[uno|alias]] e [[Due]]"), ["due", "uno"]);
    }

    #[test]
    fn rename_links_keeps_aliases_and_other_links() {
        let t = "vedi [[Spesa]], [[spesa|la lista]] e [[Altro]]";
        assert_eq!(rename_links(t, "Spesa", "Lista spesa").unwrap(), "vedi [[Lista spesa]], [[Lista spesa|la lista]] e [[Altro]]");
        assert!(rename_links(t, "Niente", "X").is_none());
        assert!(rename_links("[[aperto", "aperto", "X").is_none());
    }

    #[test]
    fn lists_continue_with_the_right_prefix() {
        assert_eq!(list_continuation("  - voce").unwrap().2, "  - ");
        assert_eq!(list_continuation("- [x] fatto").unwrap().2, "- [ ] ");
        assert_eq!(list_continuation("9. nove").unwrap().2, "10. ");
        assert_eq!(list_continuation("> cit").unwrap().2, "> ");
        assert!(list_continuation("testo").is_none());
    }

    #[test]
    fn snake_case_is_not_italic() {
        let text = "T\nnome_file_lungo";
        assert!(parse(text)[1].spans.iter().all(|s| !s.fmt.italic));
    }
}
