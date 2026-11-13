//! The snapshot encoding of the type layer (spec/ast.md "Snapshots",
//! spec/snap.md "Hand-written roots").
//!
//! The node arenas are state. The hash-consing tables ([`InternTable`])
//! store only ids whose keys live in the arenas, so they are rebuilt from
//! the arenas in id order; the snapshot carries only the ids of `1..len`
//! that were NOT interned (the reserved type ids and the never-interned
//! variable length array nodes), so the rebuilt table has exactly the old
//! membership and a later intern of an existing key returns the old id.
//!
//! `TypeContext`:
//!
//! | field | class | how |
//! |---|---|---|
//! | `store` | state | as data (the node arena and its side arenas) |
//! | `intern` | rebuilt | from `store` in id order; the absent ids are written |
//! | `names`, `nns` | state + rebuilt | [`KeyedTable`]: entries as data, its intern table rebuilt |
//! | `name_decls` | state | as data |
//! | `dependent_templates`, `first_dtst_names`, `exc_protos`, `arg_origins`, `expr_origins`, `alias_converted` | state | maps, as entries |
//! | `dtst_first_spelling` | state | as data (set from `-fclang-compat`; the consumer's configuration check covers it) |
//! | `args` | state + rebuilt | [`ArgTable`]: arguments and lists as data, its intern table rebuilt |
//! | `profiles` | state + rebuilt | [`ProfileTable`]: words and entries as data, its intern table rebuilt |
//! | `profile_scratch` | transient | reusable buffers, restored empty |
//! | `target`, `opts` | state | as data (configuration: the consumer checks them against its own) |
//! | `exhausted` | state | as data |
//! | `vla_star_bound` | state | as data |
//!
//! [`KeyedTable`], [`ArgTable`], [`ProfileTable`]: the entry arenas are
//! state, `intern` is rebuilt as above.
//!
//! Validation on read: every check [`InternTable::rebuild`]'s hashing and
//! the O(1) queries rely on (the node and canonical arrays of equal length
//! with every builtin id present, each node's kind known and its payload
//! index inside its side arena, prototype parameter and exception ranges
//! inside their arenas, canonical links and stored `QualType`s of the side
//! arenas inside the node arena, entry ranges inside the argument and
//! profile arenas, canonical entry links in range, absent ids ascending and
//! in range).

use kiln_snap::{Desc, Reader, Schema, Snap, SnapError, Writer};
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::args::{ArgTable, ListEntry, TemplateArg};
use super::context::{FIRST_USER_TYPE, TypeContext, TypeOptions};
use super::func::{ExcSpec, FuncProto};
use super::intern::{InternTable, KeyedEntry, KeyedTable, fx_hash};
use super::kinds::TypeKind;
use super::layout::TargetTypes;
use super::node::{Shape, Store, shape};
use super::profile::{ProfileEntry, ProfileTable};
use crate::ids::{ArgListId, DeclId, ExprId, QualType, TemplateNameId, TypeId};

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

/// Reads an absent-id list written by [`InternTable::absent`] for a table
/// over `1..len`: strictly ascending and in range.
fn get_absent<O>(r: &mut Reader<'_>, len: usize) -> Result<Vec<u32>, SnapError> {
    let absent: Vec<u32> = r.get()?;
    let ok = absent.windows(2).all(|p| p[0] < p[1])
        && absent.first().is_none_or(|&a| a >= 1)
        && absent.last().is_none_or(|&a| (a as usize) < len);
    if ok { Ok(absent) } else { Err(r.invalid::<O>("intern table ids out of order or range")) }
}

/// A `start..start + len` range inside an arena of `n` items.
fn in_arena(start: u32, len: u32, n: usize) -> bool {
    u64::from(start) + u64::from(len) <= n as u64
}

/// [`KeyedTable`] (template names, nested-name-specifiers).
impl Snap for KeyedTable {
    fn put(&self, w: &mut Writer) {
        let KeyedTable { entries, intern } = self;
        entries.put(w);
        intern.absent(entries.len()).put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let entries: Vec<KeyedEntry> = at("KeyedTable.entries", r.get())?;
        let absent = at("KeyedTable.intern", get_absent::<Self>(r, entries.len()))?;
        if entries.is_empty()
            || entries.len() > u32::MAX as usize
            || entries.iter().any(|e| e.canon as usize >= entries.len())
        {
            return Err(r.invalid::<Self>(
                "keyed table without its dummy or a canonical link out of range",
            ));
        }
        let intern =
            InternTable::rebuild(entries.len(), &absent, |i| fx_hash(&entries[i as usize].key));
        Ok(KeyedTable { entries, intern })
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .field::<Vec<KeyedEntry>>("entries")
            .field::<Vec<u32>>("intern.absent")
            .skip::<InternTable>("intern");
        if s.register::<Self>(d) {
            Vec::<KeyedEntry>::schema(s);
            Vec::<u32>::schema(s);
        }
    }
}

/// [`ArgTable`] (template argument lists).
impl Snap for ArgTable {
    fn put(&self, w: &mut Writer) {
        let ArgTable { args, lists, intern } = self;
        args.put(w);
        lists.put(w);
        intern.absent(lists.len()).put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let args: Vec<TemplateArg> = at("ArgTable.args", r.get())?;
        let lists: Vec<ListEntry> = at("ArgTable.lists", r.get())?;
        let absent = at("ArgTable.intern", get_absent::<Self>(r, lists.len()))?;
        if lists.is_empty()
            || lists.len() > u32::MAX as usize
            || lists
                .iter()
                .any(|e| !in_arena(e.start, e.len, args.len()) || e.canon as usize >= lists.len())
        {
            return Err(r.invalid::<Self>("argument list out of range"));
        }
        let intern = InternTable::rebuild(lists.len(), &absent, |i| {
            let e = &lists[i as usize];
            fx_hash(&args[e.start as usize..e.start as usize + e.len as usize])
        });
        Ok(ArgTable { args, lists, intern })
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .field::<Vec<TemplateArg>>("args")
            .field::<Vec<ListEntry>>("lists")
            .field::<Vec<u32>>("intern.absent")
            .skip::<InternTable>("intern");
        if s.register::<Self>(d) {
            Vec::<TemplateArg>::schema(s);
            Vec::<ListEntry>::schema(s);
            Vec::<u32>::schema(s);
        }
    }
}

/// [`ProfileTable`] (canonical expression profiles).
impl Snap for ProfileTable {
    fn put(&self, w: &mut Writer) {
        let ProfileTable { words, entries, intern } = self;
        words.put(w);
        entries.put(w);
        intern.absent(entries.len()).put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let words: Vec<u32> = at("ProfileTable.words", r.get())?;
        let entries: Vec<ProfileEntry> = at("ProfileTable.entries", r.get())?;
        let absent = at("ProfileTable.intern", get_absent::<Self>(r, entries.len()))?;
        if entries.is_empty()
            || entries.len() > u32::MAX as usize
            || entries.iter().any(|e| !in_arena(e.start, e.len, words.len()))
        {
            return Err(r.invalid::<Self>("profile out of range"));
        }
        let intern = InternTable::rebuild(entries.len(), &absent, |i| {
            let e = &entries[i as usize];
            fx_hash(&words[e.start as usize..e.start as usize + e.len as usize])
        });
        Ok(ProfileTable { words, entries, intern })
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .field::<Vec<u32>>("words")
            .field::<Vec<ProfileEntry>>("entries")
            .field::<Vec<u32>>("intern.absent")
            .skip::<InternTable>("intern");
        if s.register::<Self>(d) {
            Vec::<u32>::schema(s);
            Vec::<ProfileEntry>::schema(s);
        }
    }
}

impl Store {
    /// The checks of a decoded store that [`Store::key_of`] and the O(1)
    /// queries rely on (see the module docs).
    fn check_restored(&self) -> Result<(), &'static str> {
        let n = self.nodes.len();
        if n != self.canon.len() || n < FIRST_USER_TYPE as usize {
            return Err("type node and canonical arrays differ or lack the builtins");
        }
        if n > TypeId::MAX_INDEX as usize + 1 || self.pairs.is_empty() || self.triples.is_empty() {
            return Err("type arenas out of range or without their dummies");
        }
        let qt = |q: QualType| (q.ty().index() as usize) < n;
        let lists = self.type_lists.len();
        if !self.type_lists.iter().all(|&q| qt(q))
            || !self.canon.iter().all(|&q| qt(q))
            || !self.arrays.iter().all(|a| qt(a.elem))
        {
            return Err("type reference out of range");
        }
        for p in &self.protos {
            let FuncProto { ret, params, n_params, info: _, exc: _ } = *p;
            let exc_ok = match p.exc_tag() {
                ExcSpec::TAG_NOEXCEPT_EXPR => (p.exc_operand() as usize) < self.triples.len(),
                ExcSpec::TAG_DYNAMIC => self
                    .pairs
                    .get(p.exc_operand() as usize)
                    .is_some_and(|&[s, l]| in_arena(s, l, lists)),
                _ => true,
            };
            if !qt(ret) || !in_arena(params, u32::from(n_params), lists) || !exc_ok {
                return Err("function prototype out of range");
            }
        }
        for node in &self.nodes {
            if usize::from(node.kind) >= TypeKind::COUNT {
                return Err("unknown type kind");
            }
            let side = match shape(node.kind()) {
                Shape::Inline => continue,
                Shape::Pair => self.pairs.len(),
                Shape::Triple => self.triples.len(),
                Shape::Array => self.arrays.len(),
                Shape::Func => self.protos.len(),
            };
            if node.data as usize >= side {
                return Err("type payload out of range");
            }
        }
        Ok(())
    }
}

/// The type context as a snapshot root (spec/ast.md "Snapshots"); see the
/// module docs for the field classification.
impl Snap for TypeContext {
    fn put(&self, w: &mut Writer) {
        let TypeContext {
            store,
            intern,
            names,
            name_decls,
            dependent_templates,
            first_dtst_names,
            dtst_first_spelling,
            exc_protos,
            arg_origins,
            expr_origins,
            alias_converted,
            nns,
            args,
            profiles,
            profile_scratch: _,
            target,
            opts,
            exhausted,
            vla_star_bound,
        } = self;
        store.put(w);
        intern.absent(store.nodes.len()).put(w);
        names.put(w);
        name_decls.put(w);
        dependent_templates.put(w);
        first_dtst_names.put(w);
        dtst_first_spelling.put(w);
        exc_protos.put(w);
        arg_origins.put(w);
        expr_origins.put(w);
        alias_converted.put(w);
        nns.put(w);
        args.put(w);
        profiles.put(w);
        target.put(w);
        opts.put(w);
        exhausted.put(w);
        vla_star_bound.put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let store: Store = at("TypeContext.store", r.get())?;
        if let Err(why) = store.check_restored() {
            return Err(r.invalid::<Self>(why));
        }
        let absent = at("TypeContext.intern", get_absent::<Self>(r, store.nodes.len()))?;
        let names = at("TypeContext.names", r.get())?;
        let name_decls = at("TypeContext.name_decls", r.get())?;
        let dependent_templates = at("TypeContext.dependent_templates", r.get())?;
        let first_dtst_names = at("TypeContext.first_dtst_names", r.get())?;
        let dtst_first_spelling = at("TypeContext.dtst_first_spelling", r.get())?;
        let exc_protos = at("TypeContext.exc_protos", r.get())?;
        let arg_origins = at("TypeContext.arg_origins", r.get())?;
        let expr_origins = at("TypeContext.expr_origins", r.get())?;
        let alias_converted = at("TypeContext.alias_converted", r.get())?;
        let nns = at("TypeContext.nns", r.get())?;
        let args = at("TypeContext.args", r.get())?;
        let profiles = at("TypeContext.profiles", r.get())?;
        let target = at("TypeContext.target", r.get())?;
        let opts = at("TypeContext.opts", r.get())?;
        let exhausted = at("TypeContext.exhausted", r.get())?;
        let vla_star_bound = at("TypeContext.vla_star_bound", r.get())?;
        let intern =
            InternTable::rebuild(store.nodes.len(), &absent, |i| fx_hash(&store.key_of(i)));
        Ok(TypeContext {
            store,
            intern,
            names,
            name_decls,
            dependent_templates,
            first_dtst_names,
            dtst_first_spelling,
            exc_protos,
            arg_origins,
            expr_origins,
            alias_converted,
            nns,
            args,
            profiles,
            profile_scratch: super::profile_expr::ProfileScratch::default(),
            target,
            opts,
            exhausted,
            vla_star_bound,
        })
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .field::<Store>("store")
            .field::<Vec<u32>>("intern.absent")
            .skip::<InternTable>("intern")
            .field::<KeyedTable>("names")
            .field::<Vec<DeclId>>("name_decls")
            .field::<FxHashSet<DeclId>>("dependent_templates")
            .field::<FxHashMap<u32, TemplateNameId>>("first_dtst_names")
            .field::<bool>("dtst_first_spelling")
            .field::<FxHashMap<TypeId, FuncProto>>("exc_protos")
            .field::<FxHashMap<TypeId, ArgListId>>("arg_origins")
            .field::<FxHashMap<TypeId, ExprId>>("expr_origins")
            .field::<FxHashMap<TypeId, ArgListId>>("alias_converted")
            .field::<KeyedTable>("nns")
            .field::<ArgTable>("args")
            .field::<ProfileTable>("profiles")
            .skip::<super::profile_expr::ProfileScratch>("profile_scratch")
            .field::<TargetTypes>("target")
            .field::<TypeOptions>("opts")
            .field::<bool>("exhausted")
            .field::<Option<ExprId>>("vla_star_bound");
        if s.register::<Self>(d) {
            Store::schema(s);
            Vec::<u32>::schema(s);
            KeyedTable::schema(s);
            Vec::<DeclId>::schema(s);
            FxHashSet::<DeclId>::schema(s);
            FxHashMap::<u32, TemplateNameId>::schema(s);
            bool::schema(s);
            FxHashMap::<TypeId, FuncProto>::schema(s);
            FxHashMap::<TypeId, ArgListId>::schema(s);
            FxHashMap::<TypeId, ExprId>::schema(s);
            ArgTable::schema(s);
            ProfileTable::schema(s);
            TargetTypes::schema(s);
            TypeOptions::schema(s);
            Option::<ExprId>::schema(s);
        }
    }
}

impl TypeContext {
    /// What interning node `t`'s own structural key finds now: `t` for an
    /// interned node, the first interned node with an equal key otherwise,
    /// `None` when no node with that key is interned (a variable length
    /// array, a reserved id). For snapshot tests: a restored context must
    /// answer as the original did (spec/ast.md "Snapshots").
    #[doc(hidden)]
    pub fn interned_as(&self, t: TypeId) -> Option<TypeId> {
        if t.index() as usize >= self.store.nodes.len() {
            return None;
        }
        self.lookup(&self.store.key_of(t.index())).ok()
    }

    /// The number of argument lists, the empty one and the dummy included
    /// (snapshot tests: every id below it is a list).
    #[doc(hidden)]
    pub fn arg_list_count(&self) -> usize {
        self.args.lists.len()
    }
}

#[cfg(test)]
mod tests {
    use kiln_snap::Snap;

    use super::super::args::ListEntry;
    use crate::decl::{DeclNode, FunctionData, ParamData, RecordData, TypedefData, VarData};
    use crate::expr::ExprNode;
    use crate::ids::QualType;
    use crate::stmt::StmtNode;
    use crate::types::{TemplateArg, TypeNode};

    /// The records of the largest arenas (by bytes, `-fkiln-memory-report`
    /// on `<bits/stdc++.h>`) encode at a fixed size, so their arrays are
    /// one bulk copy (spec/snap.md "Fixed-size records").
    #[test]
    fn largest_records_are_fixed_size() {
        let sizes = [
            ("ExprNode", ExprNode::SIZE),
            ("DeclNode", DeclNode::SIZE),
            ("TypeNode", TypeNode::SIZE),
            ("TemplateArg", TemplateArg::SIZE),
            ("ListEntry", ListEntry::SIZE),
            ("StmtNode", StmtNode::SIZE),
            ("[u32; 3]", <[u32; 3]>::SIZE),
            ("(u32, u32)", <(u32, u32)>::SIZE),
            ("QualType", QualType::SIZE),
            ("FunctionData", FunctionData::SIZE),
            ("ParamData", ParamData::SIZE),
            ("RecordData", RecordData::SIZE),
            ("VarData", VarData::SIZE),
            ("TypedefData", TypedefData::SIZE),
        ];
        let got: Vec<_> = sizes.iter().map(|&(n, s)| format!("{n}={s:?}")).collect();
        let want = "ExprNode=Some(20) DeclNode=Some(28) TypeNode=Some(8) TemplateArg=Some(9) \
                    ListEntry=Some(13) StmtNode=Some(16) [u32; 3]=Some(12) (u32, u32)=Some(8) \
                    QualType=Some(4) FunctionData=Some(24) ParamData=Some(12) RecordData=Some(37) \
                    VarData=Some(24) TypedefData=Some(16)";
        assert_eq!(got.join(" "), want);
    }
}
