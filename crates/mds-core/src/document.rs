//! mdast から正規化した文書の木を作る。検証と抽出の共通の入力。

use crate::ast::parse_mdast;
use markdown::mdast::Node;

/// 検証・抽出の対象にする文書の木。
#[derive(Debug, Default)]
pub struct Document {
    /// 深さ1の見出し（題名）
    pub titles: Vec<Heading>,
    /// 最初の節より前のブロック
    pub preamble: Vec<Block>,
    /// 深さ2の節
    pub sections: Vec<Section>,
    /// 節の外に出た深さ3以上の見出し（深さ4以上は heading_level_mismatch の対象）
    pub stray_headings: Vec<Heading>,
}

#[derive(Debug)]
pub struct Heading {
    pub text: String,
    pub depth: u8,
    pub line: usize,
}

#[derive(Debug)]
pub struct Section {
    pub name: String,
    pub line: usize,
    pub blocks: Vec<Block>,
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub struct Item {
    /// 見出しの ID 部分（先頭から `:` の直前まで）
    pub id: String,
    /// 見出しの ID より後
    pub title: String,
    pub line: usize,
    pub blocks: Vec<Block>,
}

/// 文書の1ブロック。行の種別は仕様の「規則種別」に対応する。
#[derive(Debug)]
pub enum Block {
    Field {
        name: String,
        value: String,
        line: usize,
    },
    Bullet {
        text: String,
        /// リスト項目の子である継続段落。R10 では箇条書きの一部として扱う
        continuation: Vec<String>,
        line: usize,
    },
    Statement {
        text: String,
        line: usize,
    },
    Table {
        header: Vec<String>,
        rows: Vec<Vec<String>>,
        line: usize,
    },
    Code {
        lang: Option<String>,
        value: String,
        line: usize,
    },
    /// どの規則種別にも当てはまらないブロック（引用、水平線など）
    Other {
        line: usize,
    },
}

impl Document {
    /// Markdown を mdast に解析し、文書の木に組み立てる。
    pub fn parse(src: &str) -> Result<Document, String> {
        let root = parse_mdast(src)?;
        let Node::Root(root) = root else {
            return Ok(Document::default());
        };

    let mut doc = Document::default();
    let mut current_section: Option<usize> = None;
    let mut current_item: Option<usize> = None;

    for child in root.children {
        let line = child.position().map(|p| p.start.line).unwrap_or(1);
        match child {
            Node::Yaml(_) | Node::Toml(_) => continue,
            Node::Heading(h) => {
                let text = inline_text(&h.children);
                match h.depth {
                    1 => doc.titles.push(Heading { text, depth: 1, line }),
                    2 => {
                        doc.sections.push(Section {
                            name: text,
                            line,
                            blocks: Vec::new(),
                            items: Vec::new(),
                        });
                        current_section = Some(doc.sections.len() - 1);
                        current_item = None;
                    }
                    3 => match current_section {
                        Some(sec_idx) => {
                            let section = &mut doc.sections[sec_idx];
                            let (id, title) = split_item_heading(&text);
                            section.items.push(Item {
                                id,
                                title,
                                line,
                                blocks: Vec::new(),
                            });
                            current_item = Some(section.items.len() - 1);
                        }
                        None => doc
                            .stray_headings
                            .push(Heading { text, depth: 3, line }),
                    },
                    depth => doc.stray_headings.push(Heading { text, depth, line }),
                }
            }
            other => {
                let blocks = blocks_from_node(&other, src);
                if let Some(item_idx) = current_item {
                    let section = &mut doc.sections[current_section.unwrap()];
                    section.items[item_idx].blocks.extend(blocks);
                } else if let Some(sec_idx) = current_section {
                    doc.sections[sec_idx].blocks.extend(blocks);
                } else {
                    doc.preamble.extend(blocks);
                }
            }
        }
    }
    Ok(doc)
    }
}

fn blocks_from_node(node: &Node, src: &str) -> Vec<Block> {
    let line = start_line(node);
    match node {
        Node::Paragraph(_) => {
            vec![Block::Statement {
                text: raw_slice(src, node),
                line,
            }]
        }
        Node::List(list) => {
            let mut out = Vec::new();
            for child in &list.children {
                if let Node::ListItem(item) = child {
                    out.extend(blocks_from_list_item(item, src));
                }
            }
            out
        }
        Node::Table(table) => {
            let mut rows: Vec<Vec<String>> = Vec::new();
            for row in &table.children {
                if let Node::TableRow(row) = row {
                    let cells = row.children.iter().map(cell_text).collect();
                    rows.push(cells);
                }
            }
            let mut rows = rows.into_iter();
            let header = rows.next().unwrap_or_default();
            vec![Block::Table {
                header,
                rows: rows.collect(),
                line,
            }]
        }
        Node::Code(code) => vec![Block::Code {
            lang: code.lang.clone(),
            value: code.value.clone(),
            line,
        }],
        _ => vec![Block::Other { line }],
    }
}

/// リスト項目を1つ以上のブロックにする。先頭の段落がフィールド行か箇条書きかを
/// 決め、続く段落（継続段落）は箇条書きの一部にする（R10）。入れ子のリストは
/// 各項目をトップレベルの箇条書きとして扱う。
fn blocks_from_list_item(item: &markdown::mdast::ListItem, src: &str) -> Vec<Block> {
    let item_line = line_at(item.position.as_ref());
    let mut children = item.children.iter();
    let Some(first) = children.next() else {
        return Vec::new();
    };
    let text = raw_slice(src, first);
    let mut continuation: Vec<String> = Vec::new();
    let mut nested: Vec<Block> = Vec::new();
    for rest in children {
        match rest {
            Node::Paragraph(p) => continuation.push(slice_at(src, p.position.as_ref())),
            Node::List(l) => {
                for child in &l.children {
                    if let Node::ListItem(ni) = child {
                        nested.extend(blocks_from_list_item(ni, src));
                    }
                }
            }
            // フィールド行の下の継続段落と、その他のブロック（引用など）は捨てる
            _ => {}
        }
    }
    let mut out = Vec::new();
    match split_field(&text) {
        Some((name, value)) => out.push(Block::Field {
            name,
            value,
            line: item_line,
        }),
        None => out.push(Block::Bullet {
            text,
            continuation,
            line: item_line,
        }),
    }
    out.extend(nested);
    out
}

/// `- 名前: 値` の形なら名前と値に分ける。形でなければ None。
fn split_field(text: &str) -> Option<(String, String)> {
    let idx = text.find(':')?;
    let name = text[..idx].trim();
    if name.is_empty() {
        return None;
    }
    let value = text[idx + 1..].trim();
    Some((name.to_string(), value.to_string()))
}

impl Block {
    /// R10 の箇条書きの抽出要素。元の `- ` 行と継続段落を改行でつなぐ。
    /// 継続段落が複数のときは継続段落どうしを空行でつなぐ。
    pub fn bullet_element(&self) -> String {
        let Block::Bullet {
            text,
            continuation,
            ..
        } = self
        else {
            return String::new();
        };
        let mut out = format!("- {text}");
        if !continuation.is_empty() {
            out.push('\n');
            out.push_str(&continuation.join("\n\n"));
        }
        out
    }
}

/// 項目見出しを ID と題名に分ける。`:` が無ければ全体を ID にする。
fn split_item_heading(text: &str) -> (String, String) {
    match text.find(':') {
        Some(idx) => (
            text[..idx].trim().to_string(),
            text[idx + 1..].trim().to_string(),
        ),
        None => (text.trim().to_string(), String::new()),
    }
}

/// インライン要素のテキストを連結する。
fn inline_text(children: &[Node]) -> String {
    let mut out = String::new();
    for child in children {
        match child {
            Node::Text(t) => out.push_str(&t.value),
            Node::InlineCode(c) => out.push_str(&c.value),
            Node::Emphasis(e) => out.push_str(&inline_text(&e.children)),
            Node::Strong(s) => out.push_str(&inline_text(&s.children)),
            Node::Link(l) => out.push_str(&inline_text(&l.children)),
            Node::LinkReference(l) => out.push_str(&inline_text(&l.children)),
            Node::Break(_) => out.push(' '),
            _ => {}
        }
    }
    out
}

/// 表のセルのテキスト。
fn cell_text(cell: &Node) -> String {
    if let Node::TableCell(cell) = cell {
        inline_text(&cell.children)
    } else {
        String::new()
    }
}

fn start_line(node: &Node) -> usize {
    line_at(node.position())
}

/// ノードの位置が指す範囲の生テキスト。インラインの Markdown 記法を保持する。
fn raw_slice(src: &str, node: &Node) -> String {
    slice_at(src, node.position())
}

fn line_at(position: Option<&markdown::unist::Position>) -> usize {
    position.map(|p| p.start.line).unwrap_or(1)
}

fn slice_at(src: &str, position: Option<&markdown::unist::Position>) -> String {
    match position {
        Some(pos) => src[pos.start.offset..pos.end.offset].trim().to_string(),
        None => String::new(),
    }
}