/** Owned bounded HTML tokens handle ordinary quoting without another browser parser. */
use super::markup::{Attributes, entities, static_entities, syntax};
use crate::protocol::EngineResult;

/** Raw text is requested explicitly by tree construction. */
pub(super) struct Tokenizer<'a> {
    rest: &'a str,
}
/** Native identity is allocated only by the tree builder. */
pub(super) enum Token {
    Start(String, Attributes, bool),
    End(String),
    Text(String),
}
impl<'a> Tokenizer<'a> {
    /** Public parsing has already bounded source bytes. */
    pub fn new(source: &'a str) -> Self {
        Self { rest: source }
    }
    /** Comments and declarations are discarded with bounded scans. */
    pub fn next(&mut self) -> EngineResult<Option<Token>> {
        loop {
            if self.rest.is_empty() {
                return Ok(None);
            }
            if self.rest.starts_with("<!--") {
                let end = self.rest.find("-->").ok_or_else(syntax)?;
                self.rest = &self.rest[end + 3..];
                continue;
            }
            if self
                .rest
                .get(..9)
                .is_some_and(|s| s.eq_ignore_ascii_case("<!doctype"))
            {
                let end = self.rest.find('>').ok_or_else(syntax)?;
                self.rest = &self.rest[end + 1..];
                continue;
            }
            if !self.rest.starts_with('<') {
                let end = self.rest.find('<').unwrap_or(self.rest.len());
                let text = static_entities(&self.rest[..end])?;
                self.rest = &self.rest[end..];
                return Ok(Some(Token::Text(text)));
            }
            return self.tag().map(Some);
        }
    }
    /** Quoted delimiters cannot prematurely close an attribute list. */
    fn tag(&mut self) -> EngineResult<Token> {
        self.rest = &self.rest[1..];
        let closing = self.rest.starts_with('/');
        if closing {
            self.rest = &self.rest[1..];
        }
        let end = self
            .rest
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .unwrap_or(self.rest.len());
        if end == 0
            || !self.rest[..end]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(syntax());
        }
        let name = self.rest[..end].to_ascii_lowercase();
        self.rest = &self.rest[end..];
        if closing {
            self.rest = self
                .rest
                .trim_start()
                .strip_prefix('>')
                .ok_or_else(syntax)?;
            return Ok(Token::End(name));
        }
        let mut attrs = Attributes::new();
        loop {
            self.rest = self.rest.trim_start();
            if let Some(rest) = self.rest.strip_prefix("/>") {
                self.rest = rest;
                return Ok(Token::Start(name, attrs, true));
            }
            if let Some(rest) = self.rest.strip_prefix('>') {
                self.rest = rest;
                return Ok(Token::Start(name, attrs, false));
            }
            let (key, value) = self.attribute()?;
            attrs.entry(key).or_insert(value);
        }
    }
    /** Quoted, unquoted and boolean attributes share entity decoding. */
    fn attribute(&mut self) -> EngineResult<(String, String)> {
        let end = self
            .rest
            .find(|c: char| c.is_ascii_whitespace() || "=/>".contains(c))
            .unwrap_or(self.rest.len());
        if end == 0
            || self.rest[..end]
                .chars()
                .any(|c| c.is_control() || "\"'<".contains(c))
        {
            return Err(syntax());
        }
        let name = self.rest[..end].to_ascii_lowercase();
        self.rest = self.rest[end..].trim_start();
        if !self.rest.starts_with('=') {
            return Ok((name, String::new()));
        }
        self.rest = self.rest[1..].trim_start();
        let quote = self.rest.chars().next().filter(|c| *c == '\'' || *c == '"');
        let value = if let Some(quote) = quote {
            self.rest = &self.rest[1..];
            let end = self.rest.find(quote).ok_or_else(syntax)?;
            let value = entities(&self.rest[..end])?;
            self.rest = &self.rest[end + 1..];
            value
        } else {
            let end = self
                .rest
                .find(|c: char| c.is_ascii_whitespace() || c == '>')
                .unwrap_or(self.rest.len());
            if end == 0 || self.rest[..end].contains(['<', '"', '\'', '`', '=']) {
                return Err(syntax());
            }
            let value = entities(&self.rest[..end])?;
            self.rest = &self.rest[end..];
            value
        };
        Ok((name, value))
    }
    /** Raw terminators must end at a genuine case-insensitive closing tag. */
    pub fn raw(&mut self, tag: &str) -> EngineResult<String> {
        let lower = self.rest.to_ascii_lowercase();
        let needle = format!("</{tag}");
        let end = lower
            .match_indices(&needle)
            .find(|(i, _)| {
                lower
                    .as_bytes()
                    .get(i + needle.len())
                    .is_some_and(|c| *c == b'>' || c.is_ascii_whitespace())
            })
            .map(|(i, _)| i)
            .ok_or_else(syntax)?;
        let text = self.rest[..end].to_owned();
        self.rest = &self.rest[end..];
        match self.next()? {
            Some(Token::End(name)) if name == tag => Ok(text),
            _ => Err(syntax()),
        }
    }
}
