//! The importer side of the declaration image's name gate
//! (spec/modules.md §11.11 "Part 2a", "ODR merging", WP-1162).
//!
//! An imported unit is built from its image only when none of the names
//! the image declares at namespace scope of the global namespace (its
//! *gated* names, [`crate::bmi::image_header`]) can already name a
//! declaration of the importer in that scope. The gate runs in the
//! preprocessor, ahead of the parser's lookahead, so it cannot ask sema
//! what has been declared. It records instead every identifier that
//! reached the parser *at global namespace scope*: outside any brace
//! that opens a scope of its own (a class or function body, a named
//! non-inline namespace, an initializer). A brace that opens no scope of
//! its own, or one whose names are visible in the enclosing namespace,
//! is transparent: a linkage specification (`extern "C" {`), an export
//! block (`export {`), an enumeration's body (unscoped enumerators are
//! declared in the enclosing scope; scoped ones are counted too, which
//! only costs a replay) and an inline or unnamed namespace (lookup of a
//! namespace name searches the inline namespace set, [namespace.def]/2).
//!
//! Every importer declaration of a global-scope name is such a token:
//! the declarator-id, a `using`, a namespace or class name. What else
//! the gate counts (uses, macro arguments that expand there) only costs a
//! replay. What it does not count cannot meet an image's gated
//! declaration: an identifier never returned as a token (a macro name, a
//! macro body that is not expanded, an `#if` operand, a module name), a
//! member of a class or of a named namespace (the image's declarations
//! there sit in its own scopes, reached only through a gated name), or a
//! local. The only declarations inside an opaque brace that land in the
//! global namespace are block-scope `extern` and friend declarations of
//! functions and variables; they cannot correspond to a declaration
//! attached to the imported module ([basic.link]/11, ill-formed, no
//! diagnostic required), and with other signatures they overload in
//! either case.
//!
//! The cost is one bit per symbol and a few comparisons per token, and
//! nothing when the image is off.

use kiln_lex::{Punct, TokKind, Token};
use kiln_source::symbol::{Interner, Symbol};

/// What the importer has declared so far, as far as the gate can tell.
#[derive(Debug)]
pub(crate) struct NameGate {
    /// Bit `i`: symbol `i` reached the parser at global scope (or an
    /// image that was used declares it).
    seen: Vec<u64>,
    /// The open braces, true when opaque.
    braces: Vec<bool>,
    /// How many of `braces` are opaque.
    opaque: u32,
    /// Since the last `;`, `{` or `}`: [`HEAD_ENUM`], [`HEAD_INLINE`],
    /// [`HEAD_NAMESPACE`].
    head: u8,
    /// The previous token, and whether the last token other than a
    /// string literal was `extern` with a string literal since.
    prev: Prev,
    linkage: bool,
    kw: Keywords,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prev {
    None,
    Export,
    Namespace,
    Other,
}

#[derive(Debug)]
struct Keywords {
    export: Symbol,
    extern_: Symbol,
    enum_: Symbol,
    inline: Symbol,
    namespace: Symbol,
}

const HEAD_ENUM: u8 = 1;
const HEAD_INLINE: u8 = 2;
const HEAD_NAMESPACE: u8 = 4;

impl NameGate {
    pub(crate) fn new(interner: &Interner) -> NameGate {
        let kw = Keywords {
            export: interner.intern("export"),
            extern_: interner.intern("extern"),
            enum_: interner.intern("enum"),
            inline: interner.intern("inline"),
            namespace: interner.intern("namespace"),
        };
        NameGate {
            seen: Vec::new(),
            braces: Vec::new(),
            opaque: 0,
            head: 0,
            prev: Prev::None,
            linkage: false,
            kw,
        }
    }

    /// `t` reaches the parser.
    pub(crate) fn token(&mut self, t: &Token) {
        match t.kind {
            TokKind::Identifier(s) => {
                if self.opaque == 0 {
                    self.see(s);
                }
                let k = &self.kw;
                self.linkage = s == k.extern_;
                self.prev = if s == k.export {
                    Prev::Export
                } else if s == k.namespace {
                    self.head |= HEAD_NAMESPACE;
                    Prev::Namespace
                } else {
                    if s == k.enum_ {
                        self.head |= HEAD_ENUM;
                    } else if s == k.inline {
                        self.head |= HEAD_INLINE;
                    }
                    Prev::Other
                };
            }
            // `extern "C" {` (and `extern "C" "" {`): the flag survives
            // the string literals.
            TokKind::StringLit(_) | TokKind::RawStringLit(_) => self.prev = Prev::Other,
            TokKind::Punct(Punct::LBrace) => {
                let h = self.head;
                let transparent = self.linkage
                    || self.prev == Prev::Export
                    || self.prev == Prev::Namespace
                    || h & HEAD_ENUM != 0
                    || h & (HEAD_INLINE | HEAD_NAMESPACE) == HEAD_INLINE | HEAD_NAMESPACE;
                self.braces.push(!transparent);
                self.opaque += u32::from(!transparent);
                self.reset();
            }
            TokKind::Punct(Punct::RBrace) => {
                // An unbalanced `}` (already an error) leaves the depth 0.
                if self.braces.pop() == Some(true) {
                    self.opaque -= 1;
                }
                self.reset();
            }
            TokKind::Punct(Punct::Semi) => self.reset(),
            _ => {
                self.linkage = false;
                self.prev = Prev::Other;
            }
        }
    }

    fn reset(&mut self) {
        self.head = 0;
        self.linkage = false;
        self.prev = Prev::Other;
    }

    /// From now on `s` counts as seen.
    pub(crate) fn see(&mut self, s: Symbol) {
        let i = s.index() as usize;
        let w = i / 64;
        if w >= self.seen.len() {
            self.seen.resize(w + 1, 0);
        }
        self.seen[w] |= 1 << (i % 64);
    }

    /// Whether `s` was seen.
    pub(crate) fn seen(&self, s: Symbol) -> bool {
        let i = s.index() as usize;
        self.seen.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
}
