/** Owned bounded tree construction accepts everyday server-rendered structure. */
use super::{
    Document, ScriptResource, StyleSource, markup,
    tokenizer::{Token, Tokenizer},
};
use crate::constants::{DOCUMENT_DEPTH, HTML_BYTES, NODES, SCRIPT_BYTES};
use crate::protocol::{EngineError, EngineResult};

/// Complete candidates retain stable preorder identities; unsupported active content fails.
pub fn parse(source: &str) -> EngineResult<Document> {
    if source.len() > HTML_BYTES {
        return Err(EngineError::limit("HTML bytes", HTML_BYTES));
    }
    let mut tree = Tree {
        tokens: Tokenizer::new(source),
        document: Document {
            nodes: Vec::new(),
            scripts: Vec::new(),
            title: String::new(),
            styles: Vec::new(),
            diagnostics: Vec::new(),
        },
        stack: Vec::new(),
        head: false,
    };
    while let Some(token) = tree.tokens.next()? {
        tree.consume(token)?;
    }
    super::forms::normalize(&mut tree.document)?;
    super::table::annotate(&mut tree.document)?;
    Ok(tree.document)
}
/** Optional closing recovery stays within the existing recursion budget. */
struct Tree<'a> {
    tokens: Tokenizer<'a>,
    document: Document,
    stack: Vec<usize>,
    head: bool,
}
impl Tree<'_> {
    /** Wrapper markup has no extra display node. */
    fn consume(&mut self, token: Token) -> EngineResult<()> {
        match token {
            Token::Start(tag, attrs, closed) => self.start(tag, attrs, closed),
            Token::End(tag) => {
                if tag == "head" {
                    self.head = false;
                }
                if let Some(index) = self
                    .stack
                    .iter()
                    .rposition(|id| self.document.nodes[*id].tag == tag)
                {
                    self.stack.truncate(index);
                }
                Ok(())
            }
            Token::Text(text) => self.text(text),
        }
    }
    /** Metadata/resource handling cannot create native authority. */
    fn start(&mut self, tag: String, attrs: markup::Attributes, closed: bool) -> EngineResult<()> {
        if tag == "head" {
            self.head = true;
            return Ok(());
        }
        if ["html", "body"].contains(&tag.as_str()) {
            if let Some(style) = attrs.get("style") {
                self.document
                    .styles
                    .push(StyleSource::Inline(format!("body {{{style}}}")));
            }
            return Ok(());
        }
        if tag == "title" {
            self.document.title = markup::entities(&self.tokens.raw("title")?)?;
            return Ok(());
        }
        if tag == "style" {
            let css = self.tokens.raw("style")?;
            self.document.styles.push(StyleSource::Inline(css));
            return Ok(());
        }
        if tag == "link" {
            if attrs
                .get("rel")
                .is_some_and(|s| s.eq_ignore_ascii_case("stylesheet"))
            {
                if attrs
                    .get("media")
                    .is_some_and(|s| !["all", "screen", ""].contains(&s.as_str()))
                {
                    return Err(EngineError::unsupported(
                        "Conditional stylesheets are not supported.",
                    ));
                }
                self.document.styles.push(StyleSource::Linked(
                    attrs.get("href").ok_or_else(markup::syntax)?.clone(),
                ));
            }
            return Ok(());
        }
        if tag == "meta" {
            if attrs.contains_key("http-equiv") {
                return Err(EngineError::unsupported(
                    "HTTP-equivalent metadata is not supported.",
                ));
            }
            return Ok(());
        }
        if tag == "script" {
            return self.script(attrs);
        }
        if tag == "img" {
            return self.element(tag, attrs, true);
        }
        if [
            "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
        ]
        .contains(&tag.as_str())
        {
            return self.element(tag, attrs, closed);
        }
        if self.head
            || [
                "iframe", "object", "embed", "base", "svg", "math", "video", "audio", "canvas",
                "template", "noscript",
            ]
            .contains(&tag.as_str())
        {
            return Err(EngineError::unsupported(
                "This page uses unsupported embedding or active markup.",
            ));
        }
        self.element(tag, attrs, closed)
    }
    /** V9 accepts ordered blocking classic programs; data blocks remain inert. */
    fn script(&mut self, attrs: markup::Attributes) -> EngineResult<()> {
        let source = self.tokens.raw("script")?;
        let index = self.document.scripts.len();
        if !executable(&attrs).map_err(|error| script_error(error, index))? {
            return Ok(());
        }
        if attrs.contains_key("async") || attrs.contains_key("defer") {
            return Err(script_error(
                EngineError::unsupported("Asynchronous and deferred scripts are not supported."),
                index,
            ));
        }
        if attrs.contains_key("integrity") || attrs.contains_key("crossorigin") {
            return Err(script_error(
                EngineError::unsupported(
                    "Script integrity and cross-origin modes are not supported.",
                ),
                index,
            ));
        }
        if source.len() > SCRIPT_BYTES {
            return Err(script_error(
                EngineError::limit("script bytes", SCRIPT_BYTES),
                index,
            ));
        }
        if self.document.scripts.len() >= crate::constants::SCRIPT_COUNT {
            return Err(script_error(
                EngineError::limit("classic scripts", crate::constants::SCRIPT_COUNT),
                index,
            ));
        }
        self.document.scripts.push(ScriptResource {
            source,
            src: attrs.get("src").cloned(),
        });
        Ok(())
    }
    /** Common omitted paragraph/list/option endings recover deterministically. */
    fn element(
        &mut self,
        tag: String,
        attrs: markup::Attributes,
        closed: bool,
    ) -> EngineResult<()> {
        if self.document.nodes.len() >= NODES {
            return Err(EngineError::limit("document nodes", NODES));
        }
        if ["p", "li", "option"].contains(&tag.as_str())
            && let Some(index) = self
                .stack
                .iter()
                .rposition(|id| self.document.nodes[*id].tag == tag)
        {
            self.stack.truncate(index);
        }
        if self.stack.len() >= DOCUMENT_DEPTH {
            return Err(EngineError::limit("document depth", DOCUMENT_DEPTH));
        }
        let local = self.document.nodes.len();
        let mut node = markup::node(&tag, local, self.stack.last().copied(), attrs)?;
        if tag == "textarea" {
            node.value = markup::entities(&self.tokens.raw("textarea")?)?;
            node.value = node.value.strip_prefix('\n').unwrap_or(&node.value).into();
            self.document.nodes.push(node);
            return Ok(());
        }
        self.document.nodes.push(node);
        if !closed
            && ![
                "input", "br", "hr", "wbr", "area", "source", "col", "param", "img",
            ]
            .contains(&tag.as_str())
        {
            self.stack.push(local);
        }
        Ok(())
    }
    /** Literal text stays attached to its source element where unambiguous. */
    fn text(&mut self, text: String) -> EngineResult<()> {
        if self.head || text.trim().is_empty() {
            return Ok(());
        }
        if let Some(id) = self.stack.last().copied()
            && !self.document.nodes.iter().any(|n| n.parent == Some(id))
        {
            self.document.nodes[id].text.push_str(&text);
            super::validate_text(&self.document.nodes[id].text)?;
            return Ok(());
        }
        if self.document.nodes.len() >= NODES {
            return Err(EngineError::limit("document nodes", NODES));
        }
        let mut node = markup::node(
            "#text",
            self.document.nodes.len(),
            self.stack.last().copied(),
            Default::default(),
        )?;
        node.text = text;
        self.document.nodes.push(node);
        Ok(())
    }
}

/** Module scripts are recognized explicitly; other non-JavaScript data blocks stay inert. */
fn executable(attrs: &markup::Attributes) -> EngineResult<bool> {
    let Some(kind) = attrs.get("type") else {
        return Ok(true);
    };
    let kind = kind.trim().to_ascii_lowercase();
    if kind == "module" {
        return Err(EngineError::unsupported(
            "JavaScript modules are not supported.",
        ));
    }
    Ok(kind.is_empty()
        || [
            "text/javascript",
            "application/javascript",
            "text/ecmascript",
            "application/ecmascript",
        ]
        .contains(&kind.as_str()))
}

/** Parser-level mode failures identify the script without retaining its URL or source. */
fn script_error(mut error: EngineError, index: usize) -> EngineError {
    error.location = Some(Box::new(crate::protocol::SourceLocation {
        byte_offset: 0,
        line: 1,
        column: 1,
        script_index: Some(index + 1),
    }));
    error
}
