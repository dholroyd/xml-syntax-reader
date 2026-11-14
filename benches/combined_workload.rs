//! A single deterministic document mixing every shape of input the shootout
//! workloads cover, plus constructs those workloads miss (char refs,
//! single-quoted attributes, whitespace inside tags, `]` in text, empty PIs,
//! a DOCTYPE with external ID and internal subset).
//!
//! Intended for regression detection, not attribution: one number that moves
//! when any parser path gets slower.

use std::io::Write as _;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len() as u32) as usize]
    }
}

const PROLOG: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<!DOCTYPE root PUBLIC "-//Example//DTD Bench 1.0//EN" "http://example.com/bench.dtd" [
  <!ELEMENT root ANY>
  <!ATTLIST item id CDATA #IMPLIED kind (a|b) 'a'>
  <!ENTITY company "Example &amp; Co">
  <!-- comment in subset -->
  <?subset-pi data?>
]>
<root xmlns="http://example.com/default" xmlns:a="http://example.com/a" xmlns:b="http://example.com/b">
"#;

/// Many small empty elements (element_heavy).
fn element_heavy(doc: &mut Vec<u8>, i: u32) {
    for j in 0..40 {
        write!(doc, "<e{}/>\n", i * 40 + j).unwrap();
    }
}

/// Long text runs between simple tags (text_heavy).
fn text_heavy(doc: &mut Vec<u8>) {
    let chunk = "abcdefghijklmnopqrstuvwxyz0123456789 ".repeat(20);
    for _ in 0..3 {
        write!(doc, "<p>{chunk}</p>\n").unwrap();
    }
}

/// Long text runs with refs and empty elements after them, so the inline
/// fast path handles `&name;` and `<name/>`, plus char refs and a `]` that
/// forces the slow path for `]]>` detection.
fn prose(doc: &mut Vec<u8>, rng: &mut Lcg) {
    let sentences = [
        "The quick brown fox jumps over the lazy dog while the cat watches from the windowsill. ",
        "Streaming parsers must handle every construct at every possible buffer boundary. ",
        "Performance regressions often hide in the paths that benchmarks never exercise. ",
    ];
    doc.extend_from_slice(b"<para>");
    for k in 0..6 {
        doc.extend_from_slice(rng.pick(&sentences).as_bytes());
        doc.extend_from_slice(rng.pick(&sentences).as_bytes());
        match k % 4 {
            0 => doc.extend_from_slice(b"&amp;"),
            1 => doc.extend_from_slice(b"<br/>"),
            2 => doc.extend_from_slice(b"&#8212;&#x2014;"),
            _ => doc.extend_from_slice(b"array[i] and [[nested]] brackets"),
        }
    }
    doc.extend_from_slice(b"</para>\n");
}

/// Several attributes per element, some with entity refs (attribute_heavy).
fn attribute_heavy(doc: &mut Vec<u8>, i: u32) {
    for j in 0..6 {
        let n = i * 6 + j;
        if n % 3 == 0 {
            write!(
                doc,
                r#"<item id="{n}" class="a&amp;b" data-value="x&lt;{n}&gt;y" enabled="true"/>"#,
            )
            .unwrap();
        } else {
            write!(
                doc,
                r#"<item id="{n}" class="row" data-value="xyz{n}" enabled="true"/>"#,
            )
            .unwrap();
        }
        doc.push(b'\n');
    }
}

/// Prefixed element and attribute names (namespace_heavy).
fn namespace_heavy(doc: &mut Vec<u8>, i: u32) {
    for j in 0..6 {
        let n = i * 6 + j;
        write!(doc, r#"<a:item a:id="{n}" b:val="x{n}">text</a:item>"#).unwrap();
        doc.push(b'\n');
    }
}

/// Comments, PIs, CDATA, entity refs (mixed).
fn mixed(doc: &mut Vec<u8>, i: u32) {
    write!(doc, "<!-- comment {i} -->\n").unwrap();
    write!(doc, "<?pi target data-{i}?>\n").unwrap();
    write!(doc, "<![CDATA[raw content {i}]]>\n").unwrap();
    write!(doc, "<e{i} a=\"v\">text &amp; more</e{i}>\n").unwrap();
}

/// Multi-byte UTF-8 in names, attribute values and text (utf8_heavy).
fn utf8_heavy(doc: &mut Vec<u8>, rng: &mut Lcg) {
    let names = ["título", "名前", "δεδομένα", "élément", "данные", "项目"];
    let texts = [
        "Größenänderung der Straße",
        "東京都の天気は晴れです",
        "Καλημέρα κόσμε",
        "Ça coûte 5 € — très cher",
        "Привет, мир 🌍🚀",
        "混合 mixed テキスト text",
    ];
    for _ in 0..4 {
        let name = rng.pick(&names);
        let attr = rng.pick(&texts);
        let text = rng.pick(&texts);
        write!(doc, "  <{name} attr=\"{attr}\">{text}</{name}>\n").unwrap();
    }
}

/// Indented, randomly nested config/feed-like subtree (pretty).
fn pretty(doc: &mut Vec<u8>, rng: &mut Lcg) {
    let tags = ["entry", "item", "property", "link", "author", "category", "section", "value"];
    let attrs = ["id", "name", "type", "href", "rel", "lang", "ref", "class"];
    let words = [
        "alpha", "configuration", "x", "https://example.com/path/to/resource", "true",
        "en-GB", "2026-10-16T12:00:00Z", "lorem ipsum dolor sit amet", "42", "application/xml",
    ];
    let mut stack: Vec<&str> = vec!["feed"];
    doc.extend_from_slice(b"<feed>\n");
    for _ in 0..25 {
        let indent = "  ".repeat(stack.len());
        match rng.below(10) {
            0..=3 if stack.len() < 8 => {
                let tag = rng.pick(&tags);
                write!(doc, "{indent}<{tag}").unwrap();
                for _ in 0..rng.below(4) {
                    write!(doc, " {}=\"{}\"", rng.pick(&attrs), rng.pick(&words)).unwrap();
                }
                doc.extend_from_slice(b">\n");
                stack.push(tag);
            }
            4..=5 if stack.len() > 1 => {
                let tag = stack.pop().unwrap();
                let indent = "  ".repeat(stack.len());
                write!(doc, "{indent}</{tag}>\n").unwrap();
            }
            6 => write!(doc, "{indent}<!-- {} -->\n", rng.pick(&words)).unwrap(),
            _ => {
                let tag = rng.pick(&tags);
                if rng.below(3) == 0 {
                    write!(doc, "{indent}<{tag} {}=\"{}\"/>\n", rng.pick(&attrs), rng.pick(&words))
                        .unwrap();
                } else {
                    let text = rng.pick(&words);
                    let amp = if rng.below(5) == 0 { " &amp; more" } else { "" };
                    write!(doc, "{indent}<{tag}>{text}{amp}</{tag}>\n").unwrap();
                }
            }
        }
    }
    while let Some(tag) = stack.pop() {
        let indent = "  ".repeat(stack.len());
        write!(doc, "{indent}</{tag}>\n").unwrap();
    }
}

/// Long comment/CDATA/PI bodies with near-miss delimiters (long_bodies).
fn long_bodies(doc: &mut Vec<u8>, i: u32) {
    let filler = "some text - with dashes ] and brackets ? and marks > and gts; ".repeat(8);
    write!(doc, "<!-- {i} {filler} -->\n").unwrap();
    write!(doc, "<![CDATA[{i} {filler} <not-a-tag> ]]>\n").unwrap();
    write!(doc, "<?proc {i} {filler}?>\n").unwrap();
    write!(doc, "<?proc {i} a?b ?? c??>\n").unwrap();
}

/// Less common but valid tag syntax.
fn quirky_markup(doc: &mut Vec<u8>, i: u32) {
    write!(doc, "<q a='single' b = \"spaced\" c='&#65;&#x42;&lt;' d=\"&#x1F600;\"/>\n").unwrap();
    write!(doc, "<q id=\"{i}\" />\n").unwrap();
    write!(doc, "<q\n  multi=\"line\"\n  attrs='here'\n>x</q >\n").unwrap();
    doc.extend_from_slice(b"<?empty?>\n");
    doc.extend_from_slice(b"<t>a]b ]] c]&#93;</t>\n");
}

/// Builds a document of at least `target_bytes`, cycling through every
/// fragment generator so each construct recurs throughout the document.
pub fn gen_combined(target_bytes: usize) -> Vec<u8> {
    let mut rng = Lcg(42);
    let mut doc = Vec::with_capacity(target_bytes + 16 * 1024);
    doc.extend_from_slice(PROLOG.as_bytes());
    let mut i = 0u32;
    while doc.len() < target_bytes {
        element_heavy(&mut doc, i);
        text_heavy(&mut doc);
        prose(&mut doc, &mut rng);
        attribute_heavy(&mut doc, i);
        namespace_heavy(&mut doc, i);
        mixed(&mut doc, i);
        utf8_heavy(&mut doc, &mut rng);
        pretty(&mut doc, &mut rng);
        long_bodies(&mut doc, i);
        quirky_markup(&mut doc, i);
        i += 1;
    }
    doc.extend_from_slice(b"</root>\n");
    doc
}
