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
    /// 前置部領域（最初の節より前）に出た深さ3の見出しとその内側の行。
    /// 題名より後の見出しは宣言済みの前置部の中の未宣言の構造として、open でも
    /// undeclared_heading / undeclared_line の対象になる（R13）。題名より前の
    /// 見出しは open では許す
    pub stray_preamble_headings: Vec<StrayPreambleHeading>,
}

/// 前置部領域に出た深さ3の見出しと、その下に続く内側の行。
#[derive(Debug)]
pub struct StrayPreambleHeading {
    pub heading: Heading,
    pub blocks: Vec<Block>,
    /// 題名より前に出たか。題名より前の見出しは open では許し、前置部領域の
    /// 見出しは宣言済みの前置部の内側の未宣言の構造として open でも誤りになる（R13）
    pub before_title: bool,
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
    /// 見出しに `:` があるか。無いときは invalid_id の対象（R7）
    pub has_id_separator: bool,
    pub line: usize,
    pub blocks: Vec<Block>,
}

/// 文書の1ブロック。行の種別は仕様の「規則種別」に対応する。
#[derive(Debug)]
pub enum Block {
    Field {
        /// 元の行（マーカーとその直後の空白を含む）。抽出の1要素に使う（R10）
        line_text: String,
        /// マーカーを除いた元の行テキスト（`名前: 値` の原形）。箇条書きとして
        /// 扱うときの pattern はこの元の行に適用する（R10）
        text: String,
        /// lead 段落の1行目がマーカー行にあるか。無ければ段落全体が元の行に
        /// 無い内容として抽出要素に加わる（R10）
        lead_on_marker_line: bool,
        name: String,
        value: String,
        /// フィールド行の子である継続段落。R8 ではフィールド行の一部として扱う
        continuation: Vec<String>,
        /// 子のブロック（入れ子の箇条書き・コードブロック・表など）。R10
        children: Vec<Block>,
        line: usize,
    },
    Bullet {
        /// 元の行（マーカーとその直後の空白を含む）。抽出の1要素に使う（R10）
        line_text: String,
        text: String,
        /// lead 段落の1行目がマーカー行にあるか。無ければ段落全体が元の行に
        /// 無い内容として抽出要素に加わる（R10）
        lead_on_marker_line: bool,
        /// リスト項目の子である継続段落。R10 では箇条書きの一部として扱う
        continuation: Vec<String>,
        /// 子のブロック（入れ子の箇条書き・コードブロック・表など）。R10
        children: Vec<Block>,
        line: usize,
    },
    /// 順序付きリストの項目（`1. ` など）。箇条書きの対象外（R10）。
    /// 閉じた世界では undeclared_line になる。抽出では値に含めない。
    OrderedList {
        text: String,
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
        let mut current_stray: Option<usize> = None;

        for child in root.children {
            let line = child.position().map(|p| p.start.line).unwrap_or(1);
            match child {
                Node::Yaml(_) | Node::Toml(_) => continue,
                Node::Heading(h) => {
                    let text = inline_text(&h.children);
                    match h.depth {
                        1 => {
                            doc.titles.push(Heading {
                                text,
                                depth: 1,
                                line,
                            });
                            current_stray = None;
                        }
                        2 => {
                            doc.sections.push(Section {
                                name: text,
                                line,
                                blocks: Vec::new(),
                                items: Vec::new(),
                            });
                            current_section = Some(doc.sections.len() - 1);
                            current_item = None;
                            current_stray = None;
                        }
                        3 => match current_section {
                            Some(sec_idx) => {
                                let section = &mut doc.sections[sec_idx];
                                let (id, title, has_id_separator) = split_item_heading(&text);
                                section.items.push(Item {
                                    id,
                                    title,
                                    has_id_separator,
                                    line,
                                    blocks: Vec::new(),
                                });
                                current_item = Some(section.items.len() - 1);
                            }
                            None => {
                                // 前置部領域の深さ3の見出し。内側の行をここに集める（R13）。
                                // 題名より前に出たかどうかで open の扱いが変わる（R13）
                                doc.stray_preamble_headings.push(StrayPreambleHeading {
                                    heading: Heading {
                                        text,
                                        depth: 3,
                                        line,
                                    },
                                    blocks: Vec::new(),
                                    before_title: doc.titles.is_empty(),
                                });
                                current_stray = Some(doc.stray_preamble_headings.len() - 1);
                            }
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
                    } else if let Some(stray_idx) = current_stray {
                        doc.stray_preamble_headings[stray_idx].blocks.extend(blocks);
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
        // 画像だけの行（例: `![alt](img.png)`）は文の対象外（R9）
        Node::Paragraph(_) if is_image_only_paragraph(node) => vec![Block::Other { line }],
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
                    out.extend(blocks_from_list_item(item, src, list.ordered, false));
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
/// 決め、続く段落（継続段落）は箇条書きの一部にする（R10）。順序付きリストの
/// 項目は箇条書きの対象外で、閉じた世界では undeclared_line になる（R10）。
/// 入れ子のリストは親の箇条書きの子ブロックとして保持する（R10）。コードブロック・
/// 表などの子もブロックとして子に残し、閉じた世界の undeclared_line の対象にする。
/// 先頭がコードブロック・表などで lead の段落が無いとき、後続の段落は文として
/// 扱う（R10）。文の出現回数・規則（R9）の対象になり、閉じた世界では
/// undeclared_line になる。`preserve_indent` が真のとき、子の行の元のインデントを
/// 保ったまま元の行を取る（仕様 R10 の「そのままのインデントで含める」のため）。
fn blocks_from_list_item(
    item: &markdown::mdast::ListItem,
    src: &str,
    ordered: bool,
    preserve_indent: bool,
) -> Vec<Block> {
    let item_line = line_at(item.position.as_ref());
    let mut lead_block: Option<Block> = None;
    let mut extra: Vec<Block> = Vec::new();
    for (i, child) in item.children.iter().enumerate() {
        match child {
            // 先頭の段落だけがフィールド行・箇条書き・順序付き項目の lead になる。
            // 続く段落は継続段落として lead に付く（R10）。
            Node::Paragraph(_) => {
                let text = raw_slice(src, child);
                if i == 0 {
                    // 段落の1行目がマーカー行にあれば元の行が内容を持つ。
                    // 別の行にあれば段落全体を抽出要素に加える（R10）
                    let lead_on_marker_line = item.position.as_ref().map(|p| p.start.line)
                        == child.position().map(|p| p.start.line);
                    let line_text = original_item_line(item, src, preserve_indent);
                    lead_block = Some(if ordered {
                        // 順序付きリストは箇条書きの対象外。閉じた世界では undeclared_line（R10）
                        Block::OrderedList {
                            text,
                            continuation: Vec::new(),
                            line: item_line,
                        }
                    } else {
                        match split_field(&text) {
                            Some((name, value)) => Block::Field {
                                line_text,
                                text,
                                lead_on_marker_line,
                                name,
                                value,
                                continuation: Vec::new(),
                                children: Vec::new(),
                                line: item_line,
                            },
                            None => Block::Bullet {
                                line_text,
                                text,
                                lead_on_marker_line,
                                continuation: Vec::new(),
                                children: Vec::new(),
                                line: item_line,
                            },
                        }
                    });
                } else if lead_block.is_some() {
                    match &mut lead_block {
                        Some(Block::Field { continuation, .. })
                        | Some(Block::Bullet { continuation, .. })
                        | Some(Block::OrderedList { continuation, .. }) => continuation.push(text),
                        _ => {}
                    }
                } else {
                    // lead が無い項目（先頭がコードブロック・表など）の段落は
                    // 文として扱う。継続段落は lead に付く場合だけだから、この
                    // 段落はどこにも吸われない（R10）。画像だけの段落は
                    // blocks_from_node が文の対象外にする（R9）。
                    extra.extend(blocks_from_node(child, src));
                }
            }
            Node::List(l) => {
                let mut nested = Vec::new();
                for nested_item in &l.children {
                    if let Node::ListItem(ni) = nested_item {
                        nested.extend(blocks_from_list_item(ni, src, l.ordered, true));
                    }
                }
                match &mut lead_block {
                    // 入れ子のリストは親の子として保持する（R10）
                    Some(Block::Field { children, .. }) | Some(Block::Bullet { children, .. }) => {
                        children.extend(nested)
                    }
                    _ => extra.extend(nested),
                }
            }
            // コードブロック・表などのブロックは捨てず、子のブロックとして残す。
            // 引用・水平線などは blocks_from_node が Block::Other にして閉じた
            // 世界でも無視される（R13）。
            other => {
                let blocks = blocks_from_node(other, src);
                match &mut lead_block {
                    Some(Block::Field { children, .. }) | Some(Block::Bullet { children, .. }) => {
                        children.extend(blocks)
                    }
                    _ => extra.extend(blocks),
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(lead) = lead_block {
        out.push(lead);
    }
    out.extend(extra);
    out
}

/// 段落が画像ノードだけで構成されているか。画像だけの行は文として数えない。
fn is_image_only_paragraph(node: &Node) -> bool {
    match node {
        Node::Paragraph(p) => !p.children.is_empty() && p.children.iter().all(is_image),
        _ => false,
    }
}

/// インライン要素が画像か。
fn is_image(node: &Node) -> bool {
    matches!(node, Node::Image(_))
}

/// リスト項目の元の1行目（マーカーとその直後の空白を含む）。行頭のインデントは
/// トップレベルの箇条書きとして扱うため取り除く（R10）。子の行は `preserve_indent`
/// が真で、元のインデントを保ったままの行を取る（仕様 R10 の「そのままの
/// インデントで抽出要素に含める」のため）。行末の空白（ハード改行）は元の行の
/// 一部として残す
fn original_item_line(
    item: &markdown::mdast::ListItem,
    src: &str,
    preserve_indent: bool,
) -> String {
    let offset = item.position.as_ref().map(|p| p.start.offset).unwrap_or(0);
    let line_start = if preserve_indent {
        // マーカー行の行頭まで遡って、子のインデントを含む行を取る
        src[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0)
    } else {
        offset
    };
    let end = src[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(src.len());
    if preserve_indent {
        // 子の行は元のインデントを保ったまま返す
        src[line_start..end].to_string()
    } else {
        src[offset..end].trim_start().to_string()
    }
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
    /// 子のブロック（入れ子の箇条書き・コードブロック・表など）。箇条書きでも
    /// フィールド行でもないブロックは空を返す。
    pub fn children(&self) -> &[Block] {
        match self {
            Block::Field { children, .. } | Block::Bullet { children, .. } => children,
            _ => &[],
        }
    }

    /// R10 の箇条書きの抽出要素の基本部分。元の行（マーカーとその直後の空白を含む）
    /// と継続段落を改行でつなぐ。継続段落が複数のときは継続段落どうしを空行で
    /// つなぐ。子の箇条書きの行の連結は、宣言された子フィールドを除外する必要が
    /// あるため extract 側の `element_with_children` が行う（R10・R16）。
    pub fn bullet_element(&self) -> String {
        let Block::Bullet {
            line_text,
            text,
            lead_on_marker_line,
            continuation,
            ..
        } = self
        else {
            return String::new();
        };
        element_from_line(line_text, text, *lead_on_marker_line, continuation)
    }

    /// フィールド行の抽出要素。元の行（マーカーとその直後の空白を含む）と
    /// 継続段落を改行でつなぐ。継続段落が複数のときは継続段落どうしを
    /// 空行でつなぐ（R8・R10・R16）。子の箇条書きの行は含めない。
    pub fn field_element(&self) -> String {
        let Block::Field {
            line_text,
            text,
            lead_on_marker_line,
            continuation,
            ..
        } = self
        else {
            return String::new();
        };
        element_from_line(line_text, text, *lead_on_marker_line, continuation)
    }
}

/// 抽出要素を組み立てる。元の行（マーカーとその直後の空白を含む）に、lead 段落の
/// うち元の行に無い内容を改行でつなぎ、続けて継続段落を改行でつなぐ（R10）。
fn element_from_line(
    line_text: &str,
    text: &str,
    lead_on_marker_line: bool,
    continuation: &[String],
) -> String {
    let mut out = line_text.to_string();
    // 段落がマーカー行に始まれば1行目は元の行が持ち、別の行に始まれば段落全体が
    // 元の行に無い内容として加わる（R10）
    let rest = if lead_on_marker_line {
        text.split_once('\n').map(|(_, r)| r).unwrap_or("")
    } else {
        text
    };
    if !rest.is_empty() {
        out.push('\n');
        out.push_str(rest);
    }
    join_continuation(&mut out, continuation);
    out
}

/// 継続段落を `- ` 行に改行で続けてつなぐ。複数あるときは空行でつなぐ（R10）。
/// フィールド行と箇条書きの抽出要素が共有する結合規則。
pub(crate) fn join_continuation(out: &mut String, continuation: &[String]) {
    if !continuation.is_empty() {
        out.push('\n');
        out.push_str(&continuation.join("\n\n"));
    }
}

/// 項目見出しを ID と題名に分ける。`:` が無ければ全体を ID にし、区切りが
/// 無いことを返す（R7 の invalid_id の判定に使う）。
fn split_item_heading(text: &str) -> (String, String, bool) {
    match text.find(':') {
        Some(idx) => (
            text[..idx].trim().to_string(),
            text[idx + 1..].trim().to_string(),
            true,
        ),
        None => (text.trim().to_string(), String::new(), false),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn section_blocks(doc: &Document) -> &[Block] {
        assert_eq!(doc.sections.len(), 1, "節が1つあること");
        &doc.sections[0].blocks
    }

    #[test]
    fn nested_list_items_become_children_of_the_parent_bullet() {
        let doc = Document::parse("## 理由\n\n- 親\n  - 子\n    - 孫\n").unwrap();
        let blocks = section_blocks(&doc);
        assert_eq!(blocks.len(), 1);
        let Block::Bullet { text, children, .. } = &blocks[0] else {
            panic!("親が Bullet になる");
        };
        assert_eq!(text, "親");
        assert_eq!(children.len(), 1);
        let Block::Bullet { text, children, .. } = &children[0] else {
            panic!("子が Bullet になる");
        };
        assert_eq!(text, "子");
        assert_eq!(children.len(), 1);
        let Block::Bullet { text, .. } = &children[0] else {
            panic!("孫が Bullet になる");
        };
        assert_eq!(text, "孫");
    }

    #[test]
    fn nested_list_children_keep_their_indentation() {
        let doc = Document::parse("## 理由\n\n- 親\n  - 子\n    - 孫\n").unwrap();
        let blocks = section_blocks(&doc);
        let Block::Bullet {
            line_text,
            children,
            ..
        } = &blocks[0]
        else {
            panic!("親が Bullet になる");
        };
        assert_eq!(line_text, "- 親");
        let Block::Bullet {
            line_text,
            children,
            ..
        } = &children[0]
        else {
            panic!("子が Bullet になる");
        };
        assert_eq!(line_text, "  - 子");
        let Block::Bullet { line_text, .. } = &children[0] else {
            panic!("孫が Bullet になる");
        };
        assert_eq!(line_text, "    - 孫");
    }

    #[test]
    fn continuation_paragraph_attaches_to_the_parent_bullet() {
        let doc = Document::parse("## 理由\n\n- 親\n\n  続きの段落\n  - 子\n").unwrap();
        let blocks = section_blocks(&doc);
        let Block::Bullet {
            continuation,
            children,
            ..
        } = &blocks[0]
        else {
            panic!("親が Bullet になる");
        };
        assert_eq!(continuation, &vec!["続きの段落".to_string()]);
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn code_block_child_of_bullet_remains_a_block() {
        let doc = Document::parse("## 理由\n\n- 親\n\n  ```python\n  x = 1\n  ```\n").unwrap();
        let blocks = section_blocks(&doc);
        let Block::Bullet { children, .. } = &blocks[0] else {
            panic!("親が Bullet になる");
        };
        assert!(matches!(children[0], Block::Code { lang: Some(ref l), .. } if l == "python"));
    }

    #[test]
    fn table_child_of_bullet_remains_a_block() {
        let doc =
            Document::parse("## 理由\n\n- 親\n\n  | a | b |\n  |---|---|\n  | 1 | 2 |\n").unwrap();
        let blocks = section_blocks(&doc);
        let Block::Bullet { children, .. } = &blocks[0] else {
            panic!("親が Bullet になる");
        };
        assert!(
            matches!(children[0], Block::Table { ref header, .. } if header == &vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn statement_after_code_block_lead_is_a_statement() {
        let doc =
            Document::parse("## 理由\n\n- ```python\n  x = 1\n  ```\n\n  後続の段落\n").unwrap();
        let blocks = section_blocks(&doc);
        assert!(matches!(blocks[0], Block::Code { .. }));
        assert!(matches!(blocks[1], Block::Statement { ref text, .. } if text == "後続の段落"));
    }

    #[test]
    fn field_line_with_children_keeps_children() {
        let doc = Document::parse("## 理由\n\n- 状態: 承認済み\n  - 子\n").unwrap();
        let blocks = section_blocks(&doc);
        let Block::Field {
            name,
            children,
            line_text,
            ..
        } = &blocks[0]
        else {
            panic!("親の `- 名前: 値` 行が Field になる");
        };
        assert_eq!(name, "状態");
        assert_eq!(line_text, "- 状態: 承認済み");
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn ordered_list_item_children_are_not_attached() {
        let doc = Document::parse("## 理由\n\n1. 順序付き\n   - 箇条書き\n").unwrap();
        let blocks = section_blocks(&doc);
        assert!(matches!(blocks[0], Block::OrderedList { .. }));
        assert!(
            matches!(blocks[1], Block::Bullet { .. }),
            "順序付きの入れ子は対象外でトップレベルに残る"
        );
    }
}
