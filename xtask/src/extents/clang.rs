//! Clang's side of the extent differential (spec/harness.md "Expression
//! extents"): the source ranges of the expressions of clang's
//! `-ast-dump=json`, read as a stream.
//!
//! Clang leaves a location's `file` out when it is the file of the
//! location it printed last, in document order, so the reader carries the
//! last file forward over every location of the dump, kept or not. A
//! location object is file text when it has `offset` itself; a macro
//! location has `spellingLoc` and `expansionLoc` objects instead.

use std::collections::HashMap;
use std::io::{self, Read};

use super::json::{Event, Reader};

/// A location of the file text: the file (an index into [`Dump::files`]),
/// the byte offset and the length of the token there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bare {
    pub file: u32,
    pub offset: u32,
    pub tok_len: u32,
}

/// A location as clang prints it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CLoc {
    File(Bare),
    Macro {
        spelling: Option<Bare>,
        expansion: Option<Bare>,
        /// `isMacroArgExpansion` (on the expansion location).
        arg: bool,
    },
}

/// One expression node of a written body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: String,
    pub begin: Option<CLoc>,
    pub end: Option<CLoc>,
}

/// What the reader kept of one dump.
#[derive(Debug, Default)]
pub struct Dump {
    /// The file names, as clang printed them.
    pub files: Vec<String>,
    pub nodes: Vec<Node>,
}

/// The template declaration whose children a node is among: its first
/// templated declaration is the pattern, later ones are specializations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parent {
    Other,
    FunctionTemplate { seen: bool },
    ClassTemplate { seen: bool },
    VarTemplate { seen: bool },
}

impl Parent {
    fn of(kind: &str) -> Parent {
        match kind {
            "FunctionTemplateDecl" => Parent::FunctionTemplate { seen: false },
            "ClassTemplateDecl" => Parent::ClassTemplate { seen: false },
            "VarTemplateDecl" => Parent::VarTemplate { seen: false },
            _ => Parent::Other,
        }
    }

    /// Whether the child of kind `kind` is written (not a specialization
    /// the template prints under itself).
    fn admits(&mut self, kind: &str) -> bool {
        let (seen, templated) = match self {
            Parent::Other => return true,
            Parent::FunctionTemplate { seen } => (
                seen,
                matches!(
                    kind,
                    "FunctionDecl"
                        | "CXXMethodDecl"
                        | "CXXConstructorDecl"
                        | "CXXDestructorDecl"
                        | "CXXConversionDecl"
                        | "CXXDeductionGuideDecl"
                ),
            ),
            Parent::ClassTemplate { seen } => {
                if kind == "ClassTemplateSpecializationDecl" {
                    return false;
                }
                (seen, kind == "CXXRecordDecl")
            }
            Parent::VarTemplate { seen } => {
                if kind == "VarTemplateSpecializationDecl" {
                    return false;
                }
                (seen, kind == "VarDecl")
            }
        };
        if !templated {
            return true;
        }
        !std::mem::replace(seen, true)
    }
}

struct Walker<'k, R> {
    rd: Reader<R>,
    files: Vec<String>,
    ids: HashMap<String, u32>,
    /// The file of the location printed last.
    carry: Option<u32>,
    nodes: Vec<Node>,
    is_expr: &'k dyn Fn(&str) -> bool,
}

/// Reads the JSON dump `src`, keeping the nodes whose kind `is_expr`
/// accepts under written declarations: implicit declarations and the
/// specializations a template prints under itself are skipped (their
/// locations still carry the file forward).
pub fn read(src: impl Read, is_expr: &dyn Fn(&str) -> bool) -> io::Result<Dump> {
    let mut w = Walker {
        rd: Reader::new(src),
        files: Vec::new(),
        ids: HashMap::new(),
        carry: None,
        nodes: Vec::new(),
        is_expr,
    };
    match w.rd.next()? {
        Event::Obj => w.node(true, &mut Parent::Other)?,
        Event::End => {}
        ev => w.value(&ev)?,
    }
    Ok(Dump { files: w.files, nodes: w.nodes })
}

fn data(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("clang's JSON dump: {what}"))
}

impl<R: Read> Walker<'_, R> {
    fn file_id(&mut self, name: String) -> u32 {
        if let Some(&i) = self.ids.get(&name) {
            return i;
        }
        let i = u32::try_from(self.files.len()).unwrap_or(u32::MAX);
        self.files.push(name.clone());
        self.ids.insert(name, i);
        i
    }

    /// An AST node (its `{` read): `keep` is false inside a skipped
    /// subtree; `parent` is the context of the node's parent.
    fn node(&mut self, keep: bool, parent: &mut Parent) -> io::Result<()> {
        let mut kind = String::new();
        let mut range = (None, None);
        let mut implicit = false;
        let mut admitted = true;
        let mut me = Parent::Other;
        loop {
            match self.rd.next()? {
                Event::EndObj => break,
                Event::Key(k) => match k.as_str() {
                    "kind" => {
                        if let Event::Str(s) = self.rd.next()? {
                            admitted = parent.admits(&s);
                            me = Parent::of(&s);
                            kind = s;
                        }
                    }
                    "range" => {
                        let ev = self.rd.next()?;
                        range = self.range(&ev)?;
                    }
                    "isImplicit" => {
                        let ev = self.rd.next()?;
                        implicit = ev == Event::Bool(true);
                    }
                    // An initializer list with an array filler prints the
                    // filler and then its initializers under
                    // `array_filler`.
                    "inner" | "array_filler" => {
                        let ev = self.rd.next()?;
                        if ev != Event::Arr {
                            self.value(&ev)?;
                            continue;
                        }
                        let kids = keep && admitted && !implicit;
                        loop {
                            match self.rd.next()? {
                                Event::EndArr => break,
                                Event::Obj => self.node(kids, &mut me)?,
                                ev => self.value(&ev)?,
                            }
                        }
                    }
                    _ => {
                        let ev = self.rd.next()?;
                        self.value(&ev)?;
                    }
                },
                _ => return Err(data("a node is not an object of keys")),
            }
        }
        if keep && admitted && (self.is_expr)(&kind) {
            self.nodes.push(Node { kind, begin: range.0, end: range.1 });
        }
        Ok(())
    }

    /// `range`: `{"begin": loc, "end": loc}`.
    fn range(&mut self, ev: &Event) -> io::Result<(Option<CLoc>, Option<CLoc>)> {
        if *ev != Event::Obj {
            self.value(ev)?;
            return Ok((None, None));
        }
        let mut out = (None, None);
        loop {
            match self.rd.next()? {
                Event::EndObj => break,
                Event::Key(k) => {
                    let ev = self.rd.next()?;
                    let l = self.loc(&ev)?;
                    match k.as_str() {
                        "begin" => out.0 = l,
                        "end" => out.1 = l,
                        _ => {}
                    }
                }
                _ => return Err(data("a range is not an object of keys")),
            }
        }
        Ok(out)
    }

    /// A location value.
    fn loc(&mut self, ev: &Event) -> io::Result<Option<CLoc>> {
        if *ev != Event::Obj {
            self.value(ev)?;
            return Ok(None);
        }
        let (bare, spelling, expansion, arg) = self.loc_object()?;
        Ok(match (bare, spelling, expansion) {
            (Some(b), None, None) => Some(CLoc::File(b)),
            (_, None, None) => None,
            (_, s, e) => Some(CLoc::Macro { spelling: s, expansion: e, arg }),
        })
    }

    /// The keys of a location object (its `{` read): the bare location
    /// it is (when it has `offset`), its spelling and expansion
    /// locations, and `isMacroArgExpansion`.
    #[allow(clippy::type_complexity, reason = "one private helper")]
    fn loc_object(&mut self) -> io::Result<(Option<Bare>, Option<Bare>, Option<Bare>, bool)> {
        let mut offset = None;
        let mut tok_len = 0;
        let mut own_file = None;
        let (mut spelling, mut expansion, mut arg) = (None, None, false);
        loop {
            match self.rd.next()? {
                Event::EndObj => break,
                Event::Key(k) => {
                    let ev = self.rd.next()?;
                    match (k.as_str(), ev) {
                        ("offset", Event::Num(n)) => offset = n,
                        ("file", Event::Str(f)) if offset.is_some() => {
                            let id = self.file_id(f);
                            own_file = Some(id);
                            self.carry = Some(id);
                        }
                        ("tokLen", Event::Num(n)) => tok_len = n.unwrap_or(0),
                        ("isMacroArgExpansion", Event::Bool(b)) => arg = b,
                        ("spellingLoc", Event::Obj) => {
                            let (b, ..) = self.loc_object()?;
                            spelling = b;
                        }
                        ("expansionLoc", Event::Obj) => {
                            let (b, _, _, a) = self.loc_object()?;
                            expansion = b;
                            arg |= a;
                        }
                        (_, ev) => self.value(&ev)?,
                    }
                }
                _ => return Err(data("a location is not an object of keys")),
            }
        }
        let bare = match (offset, own_file.or(self.carry)) {
            (Some(o), Some(file)) => Some(Bare {
                file,
                offset: u32::try_from(o).map_err(|_| data("an offset out of range"))?,
                tok_len: u32::try_from(tok_len).unwrap_or(0),
            }),
            _ => None,
        };
        Ok((bare, spelling, expansion, arg))
    }

    /// Any other value (its first event read): walked to its end, every
    /// location in it carrying the file forward.
    fn value(&mut self, ev: &Event) -> io::Result<()> {
        match ev {
            Event::Obj => {
                // A location object begins with `offset`; anything else is
                // walked key by key.
                let mut first = true;
                loop {
                    match self.rd.next()? {
                        Event::EndObj => return Ok(()),
                        Event::Key(k) => {
                            let ev = self.rd.next()?;
                            if first && k == "offset" {
                                // A bare location: read the rest as one.
                                return self.bare_tail();
                            }
                            first = false;
                            self.value(&ev)?;
                        }
                        _ => return Err(data("an object is not made of keys")),
                    }
                }
            }
            Event::Arr => loop {
                match self.rd.next()? {
                    Event::EndArr => return Ok(()),
                    Event::Obj => self.node(false, &mut Parent::Other)?,
                    ev => self.value(&ev)?,
                }
            },
            _ => Ok(()),
        }
    }

    /// The rest of a bare location object met outside a range (its
    /// `offset` read): only its `file` matters, to carry it forward.
    fn bare_tail(&mut self) -> io::Result<()> {
        loop {
            match self.rd.next()? {
                Event::EndObj => return Ok(()),
                Event::Key(k) => {
                    let ev = self.rd.next()?;
                    match (k.as_str(), ev) {
                        ("file", Event::Str(f)) => {
                            let id = self.file_id(f);
                            self.carry = Some(id);
                        }
                        (_, ev) => self.value(&ev)?,
                    }
                }
                _ => return Err(data("a location is not an object of keys")),
            }
        }
    }
}
