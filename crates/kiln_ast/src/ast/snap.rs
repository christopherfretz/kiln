//! The snapshot encoding of the [`Ast`] (spec/ast.md "Snapshots",
//! spec/snap.md "Hand-written roots").
//!
//! The AST is a root: its type context, constant store and string pool
//! rebuild their hash-consing tables on read (`types/snap.rs`,
//! `consts/snap.rs`, `strings.rs`), and the arenas are checked against
//! each other before the value is handed out. Every other table is plain
//! data and derives `Snap`.
//!
//! | field | class | how |
//! |---|---|---|
//! | `types` | state + rebuilt | [`TypeContext`] root: arenas as data, intern tables rebuilt |
//! | `decls`, `exprs`, `stmts`, `attrs`, `annots`, `lists` | state | as data (struct-of-arrays arenas, cold maps as entries), then checked |
//! | `strings` | state + rebuilt | [`StringPool`] root: bytes as data, intern table rebuilt |
//! | `names`, `lookup`, `redecls`, `templates`, `reflections`, `modules` | state | as data (maps as entries; `names.index` and `reflections.map` included rather than rebuilt) |
//! | `consts` | state + rebuilt | [`ConstStore`] root: pools as data, intern table rebuilt |
//! | `layouts`, `virtuals`, `vtables`, `cmp_categories` | state | as data: computed on demand, but `-fdump-ast` prints cached layouts only, and the comparison categories record a check already diagnosed, so dropping them is observable |
//! | `tu` | state | as data |
//! | `contract_semantic`, `lang_c` | state | as data (configuration: the consumer checks them against its own) |
//! | `contract_sites` | state | as data |
//! | `symbols` | state (shared) | the `Arc<Interner>`: written once per snapshot through sharing, so the restored AST shares the interner with whatever else in the same snapshot holds it |
//! | `inst` | state | the instantiation meter, as data |

use std::sync::Arc;

use kiln_snap::{Desc, Reader, Schema, Snap, SnapError, Writer};
use kiln_source::symbol::Interner;

use super::{Ast, InstantiationMeter};
use crate::attr::AttrArena;
use crate::class::{ClassCache, ClassVirtuals, CmpCategories, VTableLayout};
use crate::consts::ConstStore;
use crate::decl::DeclArena;
use crate::expr::ExprArena;
use crate::ids::{DeclId, StmtId};
use crate::lists::IdLists;
use crate::lookup::LookupTables;
use crate::names::NameTable;
use crate::redecl::RedeclTable;
use crate::reflect::ReflectionTable;
use crate::slot::ColdMap;
use crate::stmt::{ContractSemantic, ContractSite, StmtArena};
use crate::strings::StringPool;
use crate::templates::TemplateTables;
use crate::types::{LayoutCache, TypeContext};

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

/// Runs `f`, a named part of a decode that reads no bytes (a table
/// rebuild), adding its time to [`take_field_times`] while
/// [`set_field_timing`] is on.
#[inline]
pub(crate) fn timed_part<T>(part: &'static str, f: impl FnOnce() -> T) -> T {
    if !FIELD_TIMING.with(std::cell::Cell::get) {
        return f();
    }
    let started = std::time::Instant::now();
    let v = f();
    let row = (part, started.elapsed(), 0);
    FIELD_TIMES.with(|t| t.borrow_mut().push(row));
    v
}

/// Reads the field `field`, adding its decode time and size to the
/// thread's [`take_field_times`] while [`set_field_timing`] is on.
#[inline]
pub(crate) fn timed<T: Snap>(r: &mut Reader<'_>, field: &'static str) -> Result<T, SnapError> {
    if !FIELD_TIMING.with(std::cell::Cell::get) {
        return at(field, r.get());
    }
    let started = std::time::Instant::now();
    let from = r.offset();
    let v = at(field, r.get());
    let row = (field, started.elapsed(), r.offset() - from);
    FIELD_TIMES.with(|t| t.borrow_mut().push(row));
    v
}

thread_local! {
    /// Whether [`Ast`]'s decode records each field's time (the snapshot
    /// load timer, `KILN_SNAP_TIMING`): instrumentation, no semantic state.
    static FIELD_TIMING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FIELD_TIMES: std::cell::RefCell<Vec<(&'static str, std::time::Duration, usize)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Turns the per-field decode timer of [`Ast`] on or off for this thread.
pub fn set_field_timing(on: bool) {
    FIELD_TIMING.with(|c| c.set(on));
}

/// Each [`Ast`] field decoded on this thread since the last call while
/// [`set_field_timing`] was on: its name, decode time and encoded size.
#[must_use]
pub fn take_field_times() -> Vec<(&'static str, std::time::Duration, usize)> {
    FIELD_TIMES.with(|t| std::mem::take(&mut *t.borrow_mut()))
}

impl Ast {
    /// The cross-arena checks of a decoded AST (see the module docs and
    /// spec/ast.md "Snapshots").
    fn check_restored(&self) -> Result<(), &'static str> {
        self.decls.check_restored()?;
        self.exprs.check_restored()?;
        self.stmts.check_restored()?;
        self.lists.check_restored()?;
        if self.tu.index() as usize > self.decls.len() {
            return Err("translation unit declaration out of range");
        }
        Ok(())
    }
}

/// The AST as a snapshot root; see the module docs for the field
/// classification.
impl Snap for Ast {
    fn put(&self, w: &mut Writer) {
        let Ast {
            types,
            decls,
            exprs,
            stmts,
            attrs,
            annots,
            lists,
            strings,
            names,
            lookup,
            redecls,
            templates,
            consts,
            reflections,
            layouts,
            virtuals,
            vtables,
            cmp_categories,
            tu,
            modules,
            contract_semantic,
            lang_c,
            contract_sites,
            symbols,
            inst,
        } = self;
        symbols.put(w);
        types.put(w);
        decls.put(w);
        exprs.put(w);
        stmts.put(w);
        attrs.put(w);
        annots.put(w);
        lists.put(w);
        strings.put(w);
        names.put(w);
        lookup.put(w);
        redecls.put(w);
        templates.put(w);
        consts.put(w);
        reflections.put(w);
        layouts.put(w);
        virtuals.put(w);
        vtables.put(w);
        cmp_categories.put(w);
        tu.put(w);
        modules.put(w);
        contract_semantic.put(w);
        lang_c.put(w);
        contract_sites.put(w);
        inst.put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let symbols = timed(r, "Ast.symbols")?;
        let types = timed(r, "Ast.types")?;
        let decls = timed(r, "Ast.decls")?;
        let exprs = timed(r, "Ast.exprs")?;
        let stmts = timed(r, "Ast.stmts")?;
        let attrs = timed(r, "Ast.attrs")?;
        let annots = timed(r, "Ast.annots")?;
        let lists = timed(r, "Ast.lists")?;
        let strings = timed(r, "Ast.strings")?;
        let names = timed(r, "Ast.names")?;
        let lookup = timed(r, "Ast.lookup")?;
        let redecls = timed(r, "Ast.redecls")?;
        let templates = timed(r, "Ast.templates")?;
        let consts = timed(r, "Ast.consts")?;
        let reflections = timed(r, "Ast.reflections")?;
        let layouts = timed(r, "Ast.layouts")?;
        let virtuals = timed(r, "Ast.virtuals")?;
        let vtables = timed(r, "Ast.vtables")?;
        let cmp_categories = timed(r, "Ast.cmp_categories")?;
        let tu = timed(r, "Ast.tu")?;
        let modules = timed(r, "Ast.modules")?;
        let contract_semantic = timed(r, "Ast.contract_semantic")?;
        let lang_c = timed(r, "Ast.lang_c")?;
        let contract_sites = timed(r, "Ast.contract_sites")?;
        let inst = timed(r, "Ast.inst")?;
        let ast = Ast {
            types,
            decls,
            exprs,
            stmts,
            attrs,
            annots,
            lists,
            strings,
            names,
            lookup,
            redecls,
            templates,
            consts,
            reflections,
            layouts,
            virtuals,
            vtables,
            cmp_categories,
            tu,
            modules,
            contract_semantic,
            lang_c,
            contract_sites,
            symbols,
            inst,
        };
        match ast.check_restored() {
            Ok(()) => Ok(ast),
            Err(why) => Err(r.invalid::<Self>(why)),
        }
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .field::<Arc<Interner>>("symbols")
            .field::<TypeContext>("types")
            .field::<DeclArena>("decls")
            .field::<ExprArena>("exprs")
            .field::<StmtArena>("stmts")
            .field::<AttrArena>("attrs")
            .field::<crate::annot::AnnotArena>("annots")
            .field::<IdLists>("lists")
            .field::<StringPool>("strings")
            .field::<NameTable>("names")
            .field::<LookupTables>("lookup")
            .field::<RedeclTable>("redecls")
            .field::<TemplateTables>("templates")
            .field::<ConstStore>("consts")
            .field::<ReflectionTable>("reflections")
            .field::<LayoutCache>("layouts")
            .field::<ClassCache<ClassVirtuals>>("virtuals")
            .field::<ClassCache<VTableLayout>>("vtables")
            .field::<CmpCategories>("cmp_categories")
            .field::<DeclId>("tu")
            .field::<crate::modules::ModuleTable>("modules")
            .field::<ContractSemantic>("contract_semantic")
            .field::<bool>("lang_c")
            .field::<ColdMap<StmtId, ContractSite>>("contract_sites")
            .field::<InstantiationMeter>("inst");
        if s.register::<Self>(d) {
            Arc::<Interner>::schema(s);
            TypeContext::schema(s);
            DeclArena::schema(s);
            ExprArena::schema(s);
            StmtArena::schema(s);
            AttrArena::schema(s);
            crate::annot::AnnotArena::schema(s);
            IdLists::schema(s);
            StringPool::schema(s);
            NameTable::schema(s);
            LookupTables::schema(s);
            RedeclTable::schema(s);
            TemplateTables::schema(s);
            ConstStore::schema(s);
            ReflectionTable::schema(s);
            LayoutCache::schema(s);
            ClassCache::<ClassVirtuals>::schema(s);
            ClassCache::<VTableLayout>::schema(s);
            CmpCategories::schema(s);
            DeclId::schema(s);
            crate::modules::ModuleTable::schema(s);
            ContractSemantic::schema(s);
            bool::schema(s);
            ColdMap::<StmtId, ContractSite>::schema(s);
            InstantiationMeter::schema(s);
        }
    }
}
