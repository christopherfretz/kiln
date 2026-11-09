//! C++20 modules in the parser (spec/modules.md): module directive events,
//! export declarations ([module.interface]), and the cross-unit
//! deduplication of namespace-scope declarations by source position
//! (spec/modules.md §6).

use kiln_ast::DeclId;
use kiln_lex::{Punct, TokKind};
use kiln_pp::DirectiveKind;
use kiln_sema::modules::Directive;
use kiln_source::fx::FxHashMap;
use kiln_source::{FileId, Loc};

use crate::parser::Parser;
use crate::tokens::Event;

/// A token's position, stable across re-inclusions of its file: the
/// (file, offset) of its expansion and spelling locations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    exp: (FileId, u32),
    spell: (FileId, u32),
}

/// One parsed namespace-scope declaration.
#[derive(Clone, Copy, Debug)]
struct Rec {
    end: Key,
    ntoks: u32,
    lo: u32,
    hi: u32,
    unit: u16,
}

/// The parser's module state (present once modules are active).
#[derive(Debug, Default)]
pub(crate) struct ParseModules {
    /// Module events not yet handed to sema: they are processed at the
    /// start of the next namespace-scope declaration.
    pending: Vec<Event>,
    /// Parsed declarations by start position.
    records: FxHashMap<Key, Vec<Rec>>,
}

/// What [`Parser::module_before_decl`] saw at a declaration's start.
/// What `module_before_decl` decided about the next declaration.
pub(crate) enum BeforeDecl {
    /// Another unit already parsed this text: its tokens were consumed.
    Skipped,
    /// Parse it; `Some` when it is recorded for deduplication.
    Parse(Option<DeclStart>),
}

pub(crate) struct DeclStart {
    key: Key,
    pos: usize,
    first: DeclId,
}

impl Parser<'_, '_> {
    /// Turns module tracking on (once).
    pub(crate) fn module_activate(&mut self) {
        if self.mods.is_none() {
            self.mods = Some(Box::default());
            self.s.module_activate();
        }
    }

    /// Queues a module event for the next declaration start.
    pub(crate) fn module_event(&mut self, e: Event) {
        self.module_activate();
        if let Some(m) = &mut self.mods {
            m.pending.push(e);
        }
    }

    /// Whether module events are waiting.
    pub(crate) fn module_pending(&self) -> bool {
        self.mods.as_ref().is_some_and(|m| !m.pending.is_empty())
    }

    /// Hands the queued module events to sema.
    pub(crate) fn module_flush(&mut self) {
        let Some(m) = &mut self.mods else { return };
        if m.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut m.pending);
        for e in pending {
            match e {
                Event::ModuleDirective(d) => {
                    let lo = d.span.lo;
                    if let Some(at) = d.after_directive
                        && matches!(
                            d.kind,
                            DirectiveKind::GlobalFragment | DirectiveKind::Module { .. }
                        )
                    {
                        self.s.module_note_pp_directive(at);
                    }
                    let dir = match &d.kind {
                        DirectiveKind::Invalid => continue,
                        DirectiveKind::GlobalFragment => Directive::GlobalFragment,
                        DirectiveKind::Module { name, partition, .. } => Directive::Module {
                            name,
                            partition: partition.as_deref(),
                            kw_loc: d.kw.lo,
                        },
                        DirectiveKind::Private { span } => Directive::Private(span.lo),
                        DirectiveKind::Import { key, display, .. } => {
                            Directive::Import { key, display }
                        }
                    };
                    self.s.act_on_module_directive(lo, d.export, dir);
                }
                Event::UnitBegin { key, header, span, carried } => {
                    self.s.act_on_unit_begin(&key, header);
                    if let Some(c) = carried {
                        self.s.install_carried_constants(&c.path, &c.bytes, span.lo);
                    }
                }
                Event::UnitEnd { key } => self.s.act_on_unit_end(&key),
                other => self.on_event(other),
            }
        }
    }

    fn module_key(&self, loc: Loc) -> Option<Key> {
        let exp = self.sm.offset_of(self.sm.expansion_loc(loc))?;
        let spell = self.sm.offset_of(self.sm.spelling_loc(loc))?;
        Some(Key { exp, spell })
    }

    /// At the start of a namespace-scope declaration: processes the
    /// pending module events, then skips the declaration if another unit
    /// already parsed the same text (spec/modules.md §6).
    pub(crate) fn module_before_decl(&mut self) -> BeforeDecl {
        self.module_flush();
        if self.at_eof() {
            return BeforeDecl::Parse(None);
        }
        let t = self.tok();
        self.s.module_note_decl(t.span.lo);
        if self.in_replay() || t.kind == TokKind::Punct(Punct::Semi) {
            return BeforeDecl::Parse(None);
        }
        let Some(key) = self.module_key(t.span.lo) else { return BeforeDecl::Parse(None) };
        let unit = self.s.module_current_unit();
        let hit = self.mods.as_ref().and_then(|m| {
            m.records.get(&key)?.iter().copied().find(|r| {
                r.unit != unit && {
                    let e = self.peek(r.ntoks as usize - 1);
                    e.kind != TokKind::Eof && self.module_key(e.span.lo) == Some(r.end)
                }
            })
        });
        if let Some(r) = hit {
            for _ in 0..r.ntoks {
                self.bump();
            }
            // Every parse of the same text (a header re-entered under
            // `__need_*` macros redeclares the same typedef) becomes
            // visible: lookup may find any of them.
            let same: Vec<(u32, u32)> = self.mods.as_ref().map_or_else(Vec::new, |m| {
                m.records.get(&key).map_or_else(Vec::new, |v| {
                    v.iter()
                        .filter(|o| o.unit != unit && o.ntoks == r.ntoks && o.end == r.end)
                        .map(|o| (o.lo, o.hi))
                        .collect()
                })
            });
            for (lo, hi) in same {
                self.s.module_merge_range(DeclId::new(lo), DeclId::new(hi));
            }
            return BeforeDecl::Skipped;
        }
        BeforeDecl::Parse(Some(DeclStart {
            key,
            pos: self.token_index(),
            first: self.s.ast.decls.next_id(),
        }))
    }

    /// After a namespace-scope declaration: records it and checks the
    /// redeclarations it made.
    pub(crate) fn module_after_decl(&mut self, start: Option<DeclStart>) {
        self.s.module_check_links();
        let Some(st) = start else { return };
        let pos = self.token_index();
        if self.in_replay() || pos <= st.pos {
            return;
        }
        let Ok(ntoks) = u32::try_from(pos - st.pos) else { return };
        let end_tok = self.tok_at(pos - 1);
        let Some(end) = self.module_key(end_tok.span.lo) else { return };
        let rec = Rec {
            end,
            ntoks,
            lo: st.first.index(),
            hi: self.s.ast.decls.next_id().index(),
            unit: self.s.module_current_unit(),
        };
        if let Some(m) = &mut self.mods {
            m.records.entry(st.key).or_default().push(rec);
        }
    }

    /// `export` declaration ([module.interface]/1): `export { ... }` or
    /// `export declaration`.
    pub(crate) fn parse_export_declaration(&mut self) {
        self.module_activate();
        let loc = self.bump().span.lo;
        let e = self.s.act_on_start_export(loc);
        let block = if let Some(open) = self.eat(Punct::LBrace) {
            while !self.at(Punct::RBrace) && !self.at_eof() {
                self.parse_external_declaration();
            }
            self.expect_closing(Punct::RBrace, Punct::LBrace, open);
            true
        } else {
            self.parse_external_declaration();
            false
        };
        self.s.act_on_finish_export(e, block);
    }
}
