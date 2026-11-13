//! The signatures of a module's public items: found by `syn`, and cut out of the source text by the places of their
//! tokens, so no item is re-printed and nothing beyond `syn`'s parsing is needed. A signature runs from its first keyword
//! to the brace or semicolon that ends it: a function's without its body, a trait's header without its items, a type's
//! with its fields or variants. Comments and whitespace are normalised away, so formatting alone changes nothing.

use std::collections::BTreeMap;

use proc_macro2::{LineColumn, Span};
use syn::token::Semi;
use syn::{Fields, FnArg, ImplItem, Item, ItemImpl, ItemTrait, Safety, Signature, TraitItem, Visibility};

/// Where in a text each line starts, in bytes.
struct Places<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Places<'a> {
    fn new(text: &'a str) -> Self {
        let breaks = text.match_indices('\n').map(|(at, _)| at.saturating_add(1));
        Self { text, starts: std::iter::once(0).chain(breaks).collect() }
    }

    /// The byte at a line and character column.
    fn at(&self, place: LineColumn) -> usize {
        let start = self.starts.get(place.line.saturating_sub(1)).copied().unwrap_or(self.text.len());
        let line = self.text.get(start..).unwrap_or_default();
        start.saturating_add(line.char_indices().nth(place.column).map_or(line.len(), |(at, _)| at))
    }

    /// The text from one place up to another.
    fn between(&self, from: LineColumn, to: LineColumn) -> &'a str {
        self.text.get(self.at(from)..self.at(to)).unwrap_or_default()
    }
}

/// `text` without its line comments, and without whitespace but a single space between two characters of words, and
/// without a comma before a closing bracket, so that the same signature wrapped differently reads the same.
pub(super) fn normalised(text: &str) -> String {
    let uncommented: Vec<&str> =
        text.lines().map(|line| line.split_once("//").map_or(line, |(code, _)| code)).collect();
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    let mut out = String::new();
    let mut gap = false;
    for c in uncommented.join("\n").chars() {
        if c.is_whitespace() {
            gap = true;
            continue;
        }
        if gap && word(c) && out.chars().last().is_some_and(word) {
            out.push(' ');
        }
        if matches!(c, ')' | '>' | ']' | '}') && out.ends_with(',') {
            out.pop();
        }
        out.push(c);
        gap = false;
    }
    out
}

/// The start of a function's signature: its first qualifier, or `fn`.
fn sig_start(sig: &Signature) -> LineColumn {
    let safety = match &sig.safety {
        Safety::Safe(t) => Some(t.span),
        Safety::Unsafe(t) => Some(t.span),
        Safety::Default => None,
    };
    let spans = [
        sig.constness.as_ref().map(|t| t.span),
        sig.asyncness.as_ref().map(|t| t.span),
        safety,
        sig.abi.as_ref().map(|abi| abi.extern_token.span),
        Some(sig.fn_token.span),
    ];
    spans
        .into_iter()
        .flatten()
        .map(|span| span.start())
        .min_by_key(|at| (at.line, at.column))
        .unwrap_or(LineColumn { line: 0, column: 0 })
}

/// A semicolon's place.
fn semi(token: &Semi) -> Span {
    semi_like(token.spans)
}

/// The place of a one-character punctuation token, from its spans.
const fn semi_like([span]: [Span; 1]) -> Span {
    span
}

/// What one module's public items are: each key, such as `trait Surface` or `Surface::point`, and its signature.
struct Found<'a> {
    places: Places<'a>,
    items: BTreeMap<String, String>,
}

impl Found<'_> {
    fn add(&mut self, key: String, from: LineColumn, to: LineColumn) {
        let text = normalised(self.places.between(from, to));
        self.items.insert(key, text);
    }

    /// A function's signature up to `end`, its arguments' types without their names, so that renaming an argument
    /// changes nothing: each typed argument from its colon to its comma, and a receiver as written.
    fn add_fn(&mut self, key: String, sig: &Signature, end: LineColumn) {
        let (open, close) = (sig.paren_token.span.open(), sig.paren_token.span.close());
        let mut text = normalised(self.places.between(sig_start(sig), open.end()));
        let mut from = open.end();
        let mut args = Vec::new();
        for pair in sig.inputs.pairs() {
            let comma = pair.punct().map(|comma| semi_like(comma.spans));
            let until = comma.map_or(close.start(), |span| span.start());
            let start = match pair.value() {
                FnArg::Typed(typed) => semi_like(typed.colon_token.spans).end(),
                FnArg::Receiver(_) => from,
            };
            args.push(normalised(self.places.between(start, until)));
            from = comma.map_or(until, |span| span.end());
        }
        text.push_str(&args.join(","));
        text.push_str(&normalised(self.places.between(close.start(), end)));
        self.items.insert(key, text);
    }

    fn trait_items(&mut self, item: &ItemTrait) {
        let name = item.ident.to_string();
        self.add(format!("trait {name}"), item.trait_token.span.start(), item.brace_token.span.open().start());
        for member in &item.items {
            match member {
                TraitItem::Fn(f) => {
                    let end = f.default.as_ref().map(|b| b.brace_token.span.open()).or(f.semi_token.as_ref().map(semi));
                    let end = end.map_or_else(|| f.sig.ident.span().end(), |span| span.start());
                    self.add_fn(format!("{name}::{}", f.sig.ident), &f.sig, end);
                }
                TraitItem::Type(t) => {
                    self.add(format!("{name}::type {}", t.ident), t.type_token.span.start(), semi(&t.semi_token).end());
                }
                TraitItem::Const(c) => {
                    self.add(
                        format!("{name}::const {}", c.ident),
                        c.const_token.span.start(),
                        semi(&c.semi_token).end(),
                    );
                }
                _ => {}
            }
        }
    }

    fn impl_items(&mut self, item: &ItemImpl) {
        let header =
            normalised(self.places.between(item.impl_token.span.start(), item.brace_token.span.open().start()));
        if item.trait_.is_some() {
            self.items.insert(header.clone(), header.clone());
        }
        for member in &item.items {
            if let ImplItem::Fn(f) = member
                && matches!(f.vis, Visibility::Public(_))
            {
                self.add_fn(format!("{header}::{}", f.sig.ident), &f.sig, f.block.brace_token.span.open().start());
            }
        }
    }

    fn item(&mut self, item: &Item) {
        let public = |vis: &Visibility| matches!(vis, Visibility::Public(_));
        match item {
            Item::Trait(t) if public(&t.vis) => self.trait_items(t),
            Item::Impl(i) => self.impl_items(i),
            Item::Struct(s) if public(&s.vis) => {
                let end = match &s.fields {
                    Fields::Named(named) => named.brace_token.span.close().end(),
                    Fields::Unnamed(_) | Fields::Unit => {
                        s.semi_token.as_ref().map_or(s.ident.span().end(), |t| semi(t).end())
                    }
                };
                self.add(format!("struct {}", s.ident), s.struct_token.span.start(), end);
            }
            Item::Enum(e) if public(&e.vis) => {
                self.add(format!("enum {}", e.ident), e.enum_token.span.start(), e.brace_token.span.close().end());
            }
            Item::Type(t) if public(&t.vis) => {
                self.add(format!("type {}", t.ident), t.type_token.span.start(), semi(&t.semi_token).end());
            }
            Item::Fn(f) if public(&f.vis) => {
                self.add_fn(format!("fn {}", f.sig.ident), &f.sig, f.block.brace_token.span.open().start());
            }
            Item::Const(c) if public(&c.vis) => {
                self.add(format!("const {}", c.ident), c.const_token.span.start(), semi(&c.semi_token).end());
            }
            Item::Static(s) if public(&s.vis) => {
                self.add(format!("static {}", s.ident), s.static_token.span.start(), semi(&s.semi_token).end());
            }
            _ => {}
        }
    }
}

/// The signatures of the public items of a module's source, by key, and of the headers of its trait impls.
///
/// # Errors
///
/// What `syn` says of a source it cannot parse.
pub(super) fn signatures(text: &str) -> Result<BTreeMap<String, String>, String> {
    let file = syn::parse_file(text).map_err(|e| e.to_string())?;
    let mut found = Found { places: Places::new(text), items: BTreeMap::new() };
    for item in &file.items {
        found.item(item);
    }
    Ok(found.items)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULE: &str = "//! A module.
use std::fmt;

/// The layer.
pub trait Layer: Send + Sync {
    /// The point.
    fn point(&self, t: f64) -> f64;

    /// With a default.
    fn twice(
        &self,
        t: f64,
    ) -> f64 {
        2.0 * self.point(t)
    }

    type Out;
    const N: usize;
}

/// A box.
#[derive(Debug)]
pub struct Boxed {
    /// Low.
    pub lo: f64, // the low end
    pub hi: f64,
}

pub enum Kind { A, B(u8) }
pub(crate) fn hidden() {}
fn private() {}
pub fn shown<'a>(x: &'a str) -> &'a str { x }
pub const fn made() -> u8 { 1 }
pub type Alias = Vec<u8>;
pub const LIMIT: u32 = 3;

impl<S: Layer> fmt::Debug for Wrap<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) }
}

impl Boxed {
    pub fn width(&self) -> f64 { self.hi - self.lo }
    fn inner(&self) {}
}

#[cfg(test)]
mod tests {
    pub fn ignored() {}
}
";

    #[test]
    fn whitespace_comments_and_wrapping_do_not_count() {
        assert_eq!(normalised("fn f(\n    a: u8,\n    b: &'a mut T,\n) -> u8"), "fn f(a:u8,b:&'a mut T)->u8");
        assert_eq!(normalised("fn f(a: u8, b: &'a mut T) -> u8"), "fn f(a:u8,b:&'a mut T)->u8");
        assert_eq!(normalised("struct S { /// doc\n pub x: u8, // note\n}"), "struct S{pub x:u8}");
        assert_eq!(normalised("Vec<Vec<u8>,>"), "Vec<Vec<u8>>");
    }

    #[test]
    fn every_public_item_is_found_and_cut_at_its_body() {
        let found = signatures(MODULE).unwrap();
        let get = |key: &str| found.get(key).map(String::as_str);
        assert_eq!(get("trait Layer"), Some("trait Layer:Send+Sync"));
        assert_eq!(get("Layer::point"), Some("fn point(&self,f64)->f64"));
        assert_eq!(get("Layer::twice"), Some("fn twice(&self,f64)->f64"), "wrapped, with a trailing comma");
        assert_eq!(get("fn made"), Some("const fn made()->u8"));
        assert_eq!(get("Layer::type Out"), Some("type Out;"));
        assert_eq!(get("Layer::const N"), Some("const N:usize;"));
        assert_eq!(get("struct Boxed"), Some("struct Boxed{pub lo:f64,pub hi:f64}"));
        assert_eq!(get("enum Kind"), Some("enum Kind{A,B(u8)}"));
        assert_eq!(get("fn shown"), Some("fn shown<'a>(&'a str)->&'a str"));
        assert_eq!(get("type Alias"), Some("type Alias=Vec<u8>;"));
        assert_eq!(get("const LIMIT"), Some("const LIMIT:u32=3;"));
        assert_eq!(get("impl<S:Layer>fmt::Debug for Wrap<S>"), Some("impl<S:Layer>fmt::Debug for Wrap<S>"));
        assert_eq!(get("impl Boxed::width"), Some("fn width(&self)->f64"));
        let keys: Vec<&str> = found.keys().map(String::as_str).collect();
        for absent in ["fn hidden", "fn private", "impl Boxed::inner", "fn ignored", "fmt"] {
            assert!(!keys.iter().any(|k| *k == absent || k.ends_with(&format!("::{absent}"))), "{absent}: {keys:?}");
        }
        assert_eq!(found.len(), 13, "{keys:?}");
    }

    #[test]
    fn an_argument_renamed_or_destructured_is_the_same_signature() {
        let one = signatures("pub fn f(_tol: u8, (lo, hi): (f64, f64)) -> u8 { 0 }").unwrap();
        let other = signatures("pub fn f(\n    tol: u8,\n    span: (f64, f64),\n) -> u8 {\n    0\n}").unwrap();
        assert_eq!(one, other);
        assert_eq!(one.get("fn f").map(String::as_str), Some("fn f(u8,(f64,f64))->u8"));
        let retyped = signatures("pub fn f(tol: u16, span: (f64, f64)) -> u8 { 0 }").unwrap();
        assert_ne!(one, retyped);
    }

    #[test]
    fn a_private_item_of_any_kind_is_no_part_of_the_seam() {
        let private = "struct Hidden; type Inner = u8; const QUIET: u8 = 1; trait Secret { fn f(&self); } enum Kept { A } \
                       static STILL: u8 = 2;";
        let found = signatures(&format!("{private} pub struct Shown; pub static SHARED: u8 = 3;")).unwrap();
        assert_eq!(found.keys().map(String::as_str).collect::<Vec<_>>(), ["static SHARED", "struct Shown"]);
        assert_eq!(found.get("static SHARED").map(String::as_str), Some("static SHARED:u8=3;"));
    }

    #[test]
    fn a_space_between_words_is_kept_beside_an_underscore_or_an_apostrophe() {
        assert_eq!(normalised("fn f(mut _x: u8, y: &'a mut T) -> u8"), "fn f(mut _x:u8,y:&'a mut T)->u8");
        assert_eq!(normalised("fn g<'a>(x: impl Trait + 'a + 'b)"), "fn g<'a>(x:impl Trait+'a+'b)");
    }

    #[test]
    fn a_source_that_does_not_parse_is_refused() {
        assert!(signatures("pub fn broken(").is_err());
    }
}
