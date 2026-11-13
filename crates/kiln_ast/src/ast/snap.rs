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
        let symbols = at("Ast.symbols", r.get())?;
        let types = at("Ast.types", r.get())?;
        let decls = at("Ast.decls", r.get())?;
        let exprs = at("Ast.exprs", r.get())?;
        let stmts = at("Ast.stmts", r.get())?;
        let attrs = at("Ast.attrs", r.get())?;
        let annots = at("Ast.annots", r.get())?;
        let lists = at("Ast.lists", r.get())?;
        let strings = at("Ast.strings", r.get())?;
        let names = at("Ast.names", r.get())?;
        let lookup = at("Ast.lookup", r.get())?;
        let redecls = at("Ast.redecls", r.get())?;
        let templates = at("Ast.templates", r.get())?;
        let consts = at("Ast.consts", r.get())?;
        let reflections = at("Ast.reflections", r.get())?;
        let layouts = at("Ast.layouts", r.get())?;
        let virtuals = at("Ast.virtuals", r.get())?;
        let vtables = at("Ast.vtables", r.get())?;
        let cmp_categories = at("Ast.cmp_categories", r.get())?;
        let tu = at("Ast.tu", r.get())?;
        let modules = at("Ast.modules", r.get())?;
        let contract_semantic = at("Ast.contract_semantic", r.get())?;
        let lang_c = at("Ast.lang_c", r.get())?;
        let contract_sites = at("Ast.contract_sites", r.get())?;
        let inst = at("Ast.inst", r.get())?;
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
