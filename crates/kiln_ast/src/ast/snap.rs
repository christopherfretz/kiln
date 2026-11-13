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

use kiln_snap::{Desc, Reader, Schema, SchemaHash, Snap, SnapError, Writer};
use kiln_source::symbol::Interner;

use super::{Ast, InstantiationMeter};
use crate::attr::AttrArena;
use crate::class::{ClassCache, ClassVirtuals, CmpCategories, VTableLayout};
use crate::consts::ConstStore;
use crate::decl::DeclArena;
use crate::expr::ExprArena;
use crate::ids::{DeclId, StmtId, space};
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

/// Reads the field `field` through the snapshot load timer
/// ([`kiln_snap::timing::timed`]).
#[inline]
pub(crate) fn timed<T: Snap>(r: &mut Reader<'_>, field: &'static str) -> Result<T, SnapError> {
    kiln_snap::timing::timed(r, field)
}

pub(crate) use kiln_snap::timing::timed_part;
/// The snapshot load timer's switch and rows, shared by every root
/// (`kiln_snap::timing`).
pub use kiln_snap::timing::{set_field_timing, take_field_times};

impl Ast {
    /// The cross-arena checks of a decoded AST (see the module docs and
    /// spec/ast.md "Snapshots").
    fn check_restored(&self) -> Result<(), &'static str> {
        self.decls.check_restored()?;
        self.exprs.check_restored()?;
        self.stmts.check_restored()?;
        self.lists.check_restored()?;
        self.decls.check_restored_bases(&self.lists)?;
        self.exprs.check_restored_paths(&self.lists, self.decls.base_spec_count())?;
        if self.tu.index() as usize > self.decls.len() {
            return Err("translation unit declaration out of range");
        }
        Ok(())
    }

    /// The largest valid index of each id space ([`space`]), for the range
    /// check of decoded ids. Call it on a checked AST (its arenas keep
    /// their dummies).
    fn id_bounds(&self) -> [usize; space::COUNT] {
        let mut b = [0; space::COUNT];
        let [names, nns, args, profiles] = self.types.max_table_ids();
        b[space::DECL] = self.decls.len();
        b[space::EXPR] = self.exprs.len();
        b[space::STMT] = self.stmts.len();
        b[space::ATTR] = self.attrs.max_index();
        b[space::LIST] = self.lists.count() + 1;
        b[space::CONST_VALUE] = self.consts.len();
        // Never handed out (the constant store keeps no blob pool yet).
        b[space::BLOB] = 0;
        b[space::REFLECTION] = self.reflections.len();
        // Data member descriptions index the Oven's `DmsTable`, which the
        // AST's copy mirrors only after codegen (spec/ast.md "Snapshots",
        // links out of the AST): not checked here.
        b[space::DMS] = usize::MAX;
        b[space::ANN] = self.annots.len();
        b[space::STRING_LIT] = self.strings.len();
        b[space::REDECL_GROUP] = self.redecls.max_index();
        b[space::LAMBDA] = self.exprs.lambdas.max_index();
        b[space::TYPE] = self.types.len();
        b[space::TEMPLATE_NAME] = names;
        b[space::NNS] = nns;
        b[space::ARG_LIST] = args;
        b[space::PROFILE] = profiles;
        b[space::TEMPLATE_PARAM_LIST] = self.templates.max_param_list();
        b
    }

    /// The range check of the typed ids `r` has decoded so far
    /// (spec/ast.md "Snapshots"): every id read is at most the largest
    /// index of the arena it indexes. [`Ast`]'s decoder runs it on the AST
    /// itself; a root decoded after it from the same reader that holds ids
    /// into it (the Oven's and `Sema`'s state) runs it again once decoded.
    ///
    /// # Errors
    /// Which id space has an id out of range.
    pub fn check_ids(&self, r: &Reader<'_>) -> Result<(), &'static str> {
        const NAMES: [&str; space::COUNT] = [
            "declaration id out of range",
            "expression id out of range",
            "statement id out of range",
            "attribute id out of range",
            "id list id out of range",
            "constant value id out of range",
            "blob id out of range",
            "reflection id out of range",
            "data member description id out of range",
            "annotation id out of range",
            "string literal id out of range",
            "redeclaration group id out of range",
            "lambda id out of range",
            "type id out of range",
            "template name id out of range",
            "nested-name-specifier id out of range",
            "template argument list id out of range",
            "expression profile id out of range",
            "template parameter list id out of range",
        ];
        let bounds = self.id_bounds();
        match (0..space::COUNT).find(|&s| r.id_max(s) as usize > bounds[s]) {
            None => Ok(()),
            Some(s) => Err(NAMES[s]),
        }
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
        match ast.check_restored().and_then(|()| ast.check_ids(r)) {
            Ok(()) => Ok(ast),
            Err(why) => Err(r.invalid::<Self>(why)),
        }
    }

    const SCHEMA_HASH: u64 = SchemaHash::new("root", "Ast")
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
        .field::<InstantiationMeter>("inst")
        .finish();
    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .hash(Self::SCHEMA_HASH)
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
