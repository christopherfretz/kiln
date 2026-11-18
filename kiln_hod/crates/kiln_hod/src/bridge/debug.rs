//! Debug information on the bridge (hod's design 0004, seam K3b): kiln's
//! metadata copied into `hod_ir`'s debug tables as `hod_ll`'s debug
//! reader (its `debug.rs`) builds them from the text kiln prints.
//!
//! The printer of `hod_ir` writes node `DiId(i)` as `!i` and location
//! `LocId(l)` after the nodes, so equality needs the reader's ids, not
//! merely equal tables: every id is handed out in the reader's order.
//! That order follows the text, which numbers kiln's metadata as kiln's
//! printer does ([`numbering`], the printer's `number_metadata`): the
//! nodes in that order, each field left to right, a tuple named as a
//! node reserved at its first naming and defined after the node that
//! named it (the reader's stack, last in first out), the locations in
//! place, then `!llvm.dbg.cu`, then equal nodes merged
//! (`unique_nodes`). Each mapped id is memoized in [`Handle`]s indexed
//! by kiln's `MetadataId`.
//!
//! What the reader refuses is recorded with its own construct
//! (`debug:invalid`, `debug:DIEnumerator.wide`, the field table's); what
//! the reader drops is dropped; what would make the reader fail on the
//! text gives up on the whole module ([`Fallback`] `debug:<what>`), as
//! does a spelling kiln's printer can produce that this file does not
//! read (`debug:raw`).

use hod_ir::kit::{self, DebugRecord, DebugRecordKind};
use hod_ir::{
    ChecksumKind, DiAtom, DiConstant, DiFieldValue, DiFlags, DiId, DiKind, DiLocation, DiMapping,
    DiMark, DiNode, DiShape, DiString, EmissionKind, ExprId, LocId, NameTableKind, SpFlags, SymId,
    TypeId, di_field, di_kind, dw_ate, dw_cc, dw_lang, dw_op, dw_tag,
};
use kiln_llvm as k;

use super::{Bridge, Fallback, R, ir};

/// What a numbered node became (the reader's `debug::Handle`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Handle {
    /// Nothing (yet): a tuple no field has named as a node, or metadata
    /// that is not numbered.
    None,
    /// A debug node.
    Node(DiId),
    /// A location.
    Loc(LocId),
    /// A `DIAssignID`: nothing is kept.
    AssignId,
}

/// Which field a value read is for.
#[derive(Clone, Copy, Debug)]
enum Key {
    /// The field at this position of the kind's record.
    At(u8),
    /// One of the older spellings of a subprogram's flags, set by name.
    Name(&'static str),
}

/// A field's value before the node is built: lists are ranges of
/// `lists` until then.
#[derive(Clone, Copy, Debug)]
enum Val<'k> {
    UInt(u64),
    Int(i64),
    Bool(bool),
    Str(&'k [u8]),
    Node(Option<DiId>),
    List(u32, u32),
    Expr(ExprId),
}

/// The bridge's debug state: the handles, and the scratch storage of the
/// node being built (allocated once per module).
#[derive(Debug, Default)]
pub(super) struct DebugState<'k> {
    /// By kiln `MetadataId`; empty for a module without debug nodes.
    pub(super) handles: Vec<Handle>,
    /// The numbered nodes, in the printed order.
    order: Vec<k::MetadataId>,
    /// `!llvm.dbg.cu`'s operands (the last such named node, as the
    /// reader keeps the last).
    pub(super) units: Option<&'k [k::MetadataId]>,
    /// Tuples named as single nodes: reserved, defined after the node.
    tuples: Vec<(DiId, k::MetadataId)>,
    fields: Vec<(Key, Val<'k>)>,
    lists: Vec<DiId>,
    elems: Vec<u64>,
    /// The `!dbg` attachments of the global value being described.
    pub(super) attached: Vec<k::MetadataId>,
    /// Per attachment kind: 0 unknown yet, 1 `!dbg`, 2
    /// `!coro.outside.frame`, 3 another.
    kinds: Vec<u8>,
}

/// What a numbered kiln node is, by its kind name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Tuple,
    Location,
    AssignId,
    Node,
}

fn class(node: &k::MdNode) -> Class {
    match node {
        k::MdNode::Specialized { name: "DILocation", .. } => Class::Location,
        k::MdNode::Specialized { name: "DIAssignID", .. } => Class::AssignId,
        k::MdNode::Specialized { .. } => Class::Node,
        _ => Class::Tuple,
    }
}

/// The kinds with a record in `hod_ir` (the reader's `record_kind`).
fn record_kind(name: &str) -> Option<DiKind> {
    const RECORDS: &[&str] = &[
        "DIGlobalVariableExpression",
        "DISubrange",
        "DIEnumerator",
        "DIBasicType",
        "DIDerivedType",
        "DICompositeType",
        "DISubroutineType",
        "DIFile",
        "DICompileUnit",
        "DISubprogram",
        "DILexicalBlock",
        "DILexicalBlockFile",
        "DINamespace",
        "DITemplateTypeParameter",
        "DITemplateValueParameter",
        "DIGlobalVariable",
        "DILocalVariable",
        "DILabel",
        "DIImportedEntity",
    ];
    RECORDS.contains(&name).then(|| DiKind::named(name.as_bytes())).flatten()
}

/// The fields LLVM's parser requires (the reader's `required`).
fn required(kind: DiKind) -> &'static [&'static str] {
    match kind {
        DiKind::File => &["filename", "directory"],
        DiKind::Enumerator => &["name", "value"],
        DiKind::DerivedType => &["tag", "baseType"],
        DiKind::CompositeType => &["tag"],
        DiKind::SubroutineType => &["types"],
        DiKind::CompileUnit => &["file"],
        DiKind::LexicalBlock | DiKind::Namespace | DiKind::LocalVariable => &["scope"],
        DiKind::LexicalBlockFile => &["scope", "discriminator"],
        DiKind::TemplateTypeParameter => &["type"],
        DiKind::TemplateValueParameter => &["value"],
        DiKind::GlobalVariableExpression => &["var", "expr"],
        DiKind::ImportedEntity => &["tag", "scope"],
        DiKind::Label => &["scope", "name", "file", "line"],
        _ => &[],
    }
}

/// `DW_VIRTUALITY_*`.
fn virtuality(name: &[u8]) -> Option<u16> {
    match name {
        b"DW_VIRTUALITY_none" => Some(0),
        b"DW_VIRTUALITY_virtual" => Some(1),
        b"DW_VIRTUALITY_pure_virtual" => Some(2),
        _ => None,
    }
}

/// The number a named value of `field` stands for (the reader's
/// `named_code`).
fn named_code(shape: Option<DiShape>, field: &str, text: &[u8]) -> Option<u64> {
    let code = match shape {
        Some(DiShape::EmissionKind) => EmissionKind::named(text).map(EmissionKind::code),
        Some(DiShape::NameTableKind) => NameTableKind::named(text).map(NameTableKind::code),
        Some(DiShape::ChecksumKind) => ChecksumKind::named(text).map(ChecksumKind::code),
        _ => None,
    };
    if let Some(code) = code {
        return Some(u64::from(code));
    }
    let code = match field {
        "tag" => dw_tag(text),
        "encoding" => dw_ate(text),
        "language" | "runtimeLang" => dw_lang(text),
        "cc" => dw_cc(text),
        "virtuality" => virtuality(text),
        _ => None,
    };
    code.map(u64::from)
}

/// `A | B | 7`: `DIFlag*` (`di`) or `DISPFlag*` names and numbers.
fn flag_bits(raw: &str, di: bool) -> Option<u64> {
    let mut bits = 0u64;
    for part in raw.split(" | ") {
        bits |= if part.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
            parse_u64(part)?
        } else if di {
            u64::from(DiFlags::named(part.as_bytes())?.0)
        } else {
            u64::from(SpFlags::named(part.as_bytes())?.0)
        };
    }
    Some(bits)
}

fn parse_u64(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The older spellings of a subprogram's flags.
fn older_spelling(field: &str) -> &'static str {
    ["isLocal", "isDefinition", "isOptimized", "virtuality"]
        .into_iter()
        .find(|known| *known == field)
        .unwrap_or("")
}

/// Whether the reader's `skip_value` would call the printed value a
/// default (`0`, `false`, `null`, `""` or `!{}`).
fn printed_default(m: &k::Module, value: &k::MdField) -> bool {
    match value {
        k::MdField::Int(0)
        | k::MdField::UInt(0)
        | k::MdField::Bool(false)
        | k::MdField::Node(None) => true,
        k::MdField::Str(s) => s.is_empty(),
        k::MdField::Raw(r) => matches!(&**r, "0" | "false" | "null" | "\"\""),
        k::MdField::Node(Some(n)) => {
            // An empty tuple prints in place only when it is not
            // numbered, which kiln's printer never does.
            let _ = m.md_node(*n);
            false
        }
        _ => false,
    }
}

/// The numbered nodes of `m` in the order kiln's printer numbers them
/// (`print.rs`'s `number_metadata`): from each global's attachments, the
/// named metadata, then per function its attachments and, per
/// instruction, the metadata arguments of an intrinsic call, the debug
/// records' variables and locations and the instruction's attachments;
/// each root depth first, children in field order.
fn numbering(m: &k::Module) -> Vec<k::MetadataId> {
    let mut seen = vec![false; m.num_metadata()];
    let mut order = Vec::new();
    let mut stack = Vec::new();
    let mut add = |root: k::MetadataId| {
        stack.clear();
        stack.push(root);
        while let Some(n) = stack.pop() {
            let node = m.md_node(n);
            if !node.is_numbered() || seen[n.index()] {
                continue;
            }
            seen[n.index()] = true;
            order.push(n);
            let base = stack.len();
            match node {
                k::MdNode::Tuple { ops, .. } => stack.extend(ops.iter().flatten()),
                k::MdNode::Specialized { fields, .. } => {
                    stack.extend(fields.iter().filter_map(|(_, f)| match f {
                        k::MdField::Node(Some(n)) => Some(*n),
                        _ => None,
                    }));
                }
                _ => {}
            }
            stack[base..].reverse();
        }
    };
    for g in m.global_ids() {
        for &(_, md) in m.global(g).metadata() {
            add(md);
        }
    }
    for (_, ops) in m.named_metadata() {
        for &md in ops {
            add(md);
        }
    }
    for (_, f) in m.functions() {
        for &(_, md) in f.metadata() {
            add(md);
        }
        for &bb in f.layout() {
            for &i in f.block_insts(bb) {
                if let k::InstKind::Call(c) = *f.inst(i).kind() {
                    let call = f.call_data(c);
                    let intrinsic = matches!(call.callee().get(),
                        k::ValueRef::Func(callee) if m.func(callee).name().starts_with("llvm."));
                    if intrinsic {
                        for &a in f.ops(call.args()) {
                            if let k::ValueRef::Metadata(md) = a.get() {
                                add(md);
                            }
                        }
                    }
                }
                for r in f.debug_records(i) {
                    add(r.var);
                    add(r.loc);
                }
                for &(_, _, md) in f.inst_metadata(i) {
                    add(md);
                }
            }
        }
    }
    order
}

impl<'k> Bridge<'k> {
    /// The class of attachment `kind`: 1 `!dbg`, 2 `!coro.outside.frame`,
    /// 3 another (looked up by name once per kind).
    pub(super) fn attachment_class(&mut self, kind: k::MdKindId) -> u8 {
        let kinds = &mut self.dbg.kinds;
        if kinds.is_empty() {
            kinds.resize(self.k.num_md_kinds(), 0);
        }
        let Some(&known) = kinds.get(kind.index()) else { return 3 };
        if known != 0 {
            return known;
        }
        let class = match self.k.md_kind_name(kind) {
            "dbg" => 1,
            "coro.outside.frame" => 2,
            _ => 3,
        };
        kinds[kind.index()] = class;
        class
    }

    fn handle(&self, md: k::MetadataId) -> Handle {
        self.dbg.handles.get(md.index()).copied().unwrap_or(Handle::None)
    }

    /// `build_debug`: every numbered debug node into the module's
    /// tables, then `!llvm.dbg.cu`; nothing for a module whose numbered
    /// metadata are all tuples and that has no `!llvm.dbg.cu`.
    pub(super) fn build_debug(&mut self) -> R<()> {
        let m = self.k;
        let order = numbering(m);
        if !order.iter().any(|&n| class(m.md_node(n)) != Class::Tuple) && self.dbg.units.is_none() {
            return Ok(());
        }
        let mut handles = vec![Handle::None; m.num_metadata()];
        for &n in &order {
            handles[n.index()] = match class(m.md_node(n)) {
                Class::Tuple => Handle::None,
                Class::Location => Handle::Loc(ir(self.module.debug.reserve_loc())?),
                Class::AssignId => Handle::AssignId,
                Class::Node => Handle::Node(ir(self.module.debug.reserve())?),
            };
        }
        self.dbg.handles = handles;
        for &n in &order {
            match self.handle(n) {
                Handle::Node(_) if class(m.md_node(n)) == Class::Tuple => {}
                Handle::Node(id) => self.define_node(n, id)?,
                Handle::Loc(id) => self.define_location(n, id)?,
                Handle::None | Handle::AssignId => {}
            }
            self.define_tuples()?;
        }
        self.dbg.order = order;
        if let Some(units) = self.dbg.units {
            for &unit in units {
                let unit = self.node_ref(unit)?;
                if !ir(kit::compile_unit(&mut self.module, unit))? {
                    self.module.record_unsupported(kit::DEBUG_INVALID, None);
                }
            }
            self.define_tuples()?;
        }
        let merge = self.module.debug.unique_nodes(DiMark::default());
        if !merge.is_empty() {
            for handle in &mut self.dbg.handles {
                if let Handle::Node(id) = handle {
                    *id = merge.get(*id);
                }
            }
        }
        Ok(())
    }

    /// The tuples fields named as single nodes, and those their elements
    /// named in turn (the reader's stack).
    fn define_tuples(&mut self) -> R<()> {
        while let Some((id, md)) = self.dbg.tuples.pop() {
            let mark = self.dbg.lists.len();
            self.list_elements(md)?;
            let elements = &self.dbg.lists[mark..];
            ir(kit::define_di_tuple(&mut self.module.debug, id, elements))?;
            self.dbg.lists.truncate(mark);
        }
        Ok(())
    }

    /// Pushes the elements of the tuple `md` to `lists`.
    fn list_elements(&mut self, md: k::MetadataId) -> R<()> {
        let k::MdNode::Tuple { ops, .. } = self.k.md_node(md) else {
            return Err(Fallback::new("debug:tuple"));
        };
        for op in ops {
            let element = match op {
                Some(op) => self.node_ref(*op)?.unwrap_or(DiId::NONE),
                None => DiId::NONE,
            };
            self.dbg.lists.push(element);
        }
        Ok(())
    }

    /// `!N` where a node is expected.
    fn numbered_node(&mut self, md: k::MetadataId) -> R<DiId> {
        match self.handle(md) {
            Handle::Node(node) => Ok(node),
            Handle::None => {
                let node = ir(self.module.debug.reserve())?;
                self.dbg.handles[md.index()] = Handle::Node(node);
                self.dbg.tuples.push((node, md));
                Ok(node)
            }
            Handle::Loc(_) | Handle::AssignId => {
                self.module.record_unsupported(kit::DEBUG_INVALID, None);
                ir(kit::invalid_di_ref(&mut self.module.debug))
            }
        }
    }

    /// The single node an operand names (the reader's `node_ref`).
    fn node_ref(&mut self, md: k::MetadataId) -> R<Option<DiId>> {
        let m = self.k;
        let node = m.md_node(md);
        match node {
            k::MdNode::String(text) => {
                let node = DiNode::String(DiString { text });
                Ok(Some(ir(self.module.debug.add(&node))?))
            }
            &k::MdNode::Value(_, value) => {
                let value = self.constant(value)?;
                Ok(Some(self.constant_node(value)?))
            }
            _ if node.is_numbered() => self.numbered_node(md).map(Some),
            _ => {
                // `DIExpression` or `DIArgList` in place: not a node a
                // field may name.
                self.module.record_unsupported(kit::DEBUG_INVALID, None);
                Ok(Some(ir(kit::invalid_di_ref(&mut self.module.debug))?))
            }
        }
    }

    fn constant_node(&mut self, value: hod_ir::ConstId) -> R<DiId> {
        ir(self.module.debug.add(&DiNode::Constant(DiConstant { value })))
    }

    /// The expression of an inline `DIExpression`, interned.
    fn expression(&mut self, md: k::MetadataId) -> R<ExprId> {
        let k::MdNode::Specialized { name: "DIExpression", fields, .. } = self.k.md_node(md) else {
            return Err(Fallback::new("debug:expression"));
        };
        self.dbg.elems.clear();
        for (key, value) in fields {
            if !key.is_empty() {
                return Err(Fallback::new("debug:expression"));
            }
            match value {
                k::MdField::UInt(v) => self.dbg.elems.push(*v),
                k::MdField::Raw(raw) => {
                    if raw.is_empty() {
                        continue;
                    }
                    for element in raw.split(',') {
                        let text = element.trim_matches(' ');
                        let code = if text.starts_with("DW_OP_") {
                            dw_op(text.as_bytes())
                        } else if text.starts_with("DW_ATE_") {
                            dw_ate(text.as_bytes()).map(u64::from)
                        } else {
                            parse_u64(text)
                        };
                        let code = code.ok_or_else(|| Fallback::new("debug:expression"))?;
                        self.dbg.elems.push(code);
                    }
                }
                _ => return Err(Fallback::new("debug:expression")),
            }
        }
        ir(self.module.debug.expr_from_elements(&self.dbg.elems))
    }

    /// Defines the numbered node `md` as `id` (the reader's `node_body`
    /// and `define`).
    fn define_node(&mut self, md: k::MetadataId, id: DiId) -> R<()> {
        let k::MdNode::Specialized { distinct, name, fields } = self.k.md_node(md) else {
            return Err(Fallback::new("debug:node"));
        };
        let Some(kind) = record_kind(name) else {
            if di_kind(name.as_bytes()).is_none() {
                return Err(Fallback::new("debug:kind"));
            }
            let node = ir(kit::di_node(None, name.as_bytes(), []))?;
            return ir(kit::define_di_node(&mut self.module.debug, id, &node, *distinct));
        };
        self.dbg.fields.clear();
        self.dbg.lists.clear();
        let mut met = 0u64;
        let mut seen: Vec<&str> = Vec::new();
        let mut hint = 0;
        for (field, value) in fields {
            let index = kind.field_index(field.as_bytes(), hint);
            let again = match index {
                Some(index) => met & (1 << index) != 0,
                None => seen.contains(field),
            };
            if again {
                return Err(Fallback::new("debug:field-twice"));
            }
            if let Some(index) = index {
                met |= 1 << index;
                hint = index + 1;
                let shape = kind.field_shape_at(index);
                let value = self.field_value(kind, shape, field, value)?;
                self.dbg.fields.push((Key::At(index as u8), value));
                continue;
            }
            seen.push(field);
            match di_field(name.as_bytes(), field.as_bytes()) {
                Some(DiMapping::Mapped) => {
                    let value = self.field_value(kind, None, field, value)?;
                    self.dbg.fields.push((Key::Name(older_spelling(field)), value));
                }
                Some(DiMapping::Dropped(_)) => {}
                Some(DiMapping::Unsupported(_)) => {
                    let default = printed_default(self.k, value);
                    kit::unsupported_field(
                        &mut self.module,
                        name.as_bytes(),
                        field.as_bytes(),
                        default,
                    );
                }
                None => return Err(Fallback::new("debug:field")),
            }
        }
        for &field in required(kind) {
            let present = match kind.field_index(field.as_bytes(), 0) {
                Some(index) => met & (1 << index) != 0,
                None => seen.contains(&field),
            };
            if !present {
                return Err(Fallback::new("debug:required-field"));
            }
        }
        let mut node = DiNode::llvm_default(kind);
        let (fields, lists) = (&self.dbg.fields, &self.dbg.lists);
        for &(key, value) in fields {
            let value = match value {
                Val::UInt(v) => DiFieldValue::UInt(v),
                Val::Int(v) => DiFieldValue::Int(v),
                Val::Bool(v) => DiFieldValue::Bool(v),
                Val::Str(s) => DiFieldValue::Str(s),
                Val::Node(n) => DiFieldValue::Node(n),
                Val::List(start, end) => DiFieldValue::List(&lists[start as usize..end as usize]),
                Val::Expr(e) => DiFieldValue::Expr(e),
            };
            match key {
                Key::At(index) => ir(node.set_at(usize::from(index), value))?,
                Key::Name(field) => ir(node.set(field.as_bytes(), value))?,
            }
        }
        ir(kit::define_di_node(&mut self.module.debug, id, &node, *distinct))
    }

    /// One field's value, read by its shape (the reader's `quick_field`
    /// and `field_value`, which agree on what kiln prints).
    fn field_value(
        &mut self,
        kind: DiKind,
        shape: Option<DiShape>,
        field: &str,
        value: &'k k::MdField,
    ) -> R<Val<'k>> {
        let m = self.k;
        match shape {
            Some(DiShape::List) => {
                let mark = self.dbg.lists.len() as u32;
                match value {
                    k::MdField::Node(None) => {}
                    k::MdField::Raw(raw) if &**raw == "null" => {}
                    k::MdField::Node(Some(n)) => match m.md_node(*n) {
                        k::MdNode::Tuple { .. } => self.list_elements(*n)?,
                        k::MdNode::String(_) => return Err(Fallback::new("debug:list-shape")),
                        // A numbered node that is not a tuple, a value or
                        // a node in place: LLVM's verifier wants a tuple.
                        _ => self.module.record_unsupported(kit::DEBUG_INVALID, None),
                    },
                    _ => self.module.record_unsupported(kit::DEBUG_INVALID, None),
                }
                return Ok(Val::List(mark, self.dbg.lists.len() as u32));
            }
            Some(DiShape::Expr) => {
                return match value {
                    k::MdField::Node(Some(n)) => Ok(Val::Expr(self.expression(*n)?)),
                    _ => Err(Fallback::new("debug:expression-shape")),
                };
            }
            Some(DiShape::Flags | DiShape::SpFlags) => {
                let di = shape == Some(DiShape::Flags);
                let bits = match value {
                    k::MdField::UInt(v) => Some(*v),
                    k::MdField::Raw(raw) => flag_bits(raw, di),
                    _ => None,
                };
                return bits.map(Val::UInt).ok_or_else(|| Fallback::new("debug:flags"));
            }
            _ => {}
        }
        let node_shape = shape == Some(DiShape::Node);
        Ok(match value {
            k::MdField::Str(s) => Val::Str(s.as_bytes()),
            k::MdField::UInt(_) | k::MdField::Int(_) if node_shape => {
                return Err(Fallback::new("debug:number-for-node"));
            }
            &k::MdField::UInt(v) => Val::UInt(v),
            &k::MdField::Int(v) if v < 0 => Val::Int(v),
            &k::MdField::Int(v) => Val::UInt(v as u64),
            &k::MdField::Bool(b) => Val::Bool(b),
            k::MdField::Node(None) => Val::Node(None),
            k::MdField::Node(Some(n)) => match m.md_node(*n) {
                k::MdNode::Specialized { name: "DIExpression", .. } => {
                    Val::Expr(self.expression(*n)?)
                }
                _ => Val::Node(self.node_ref(*n)?),
            },
            k::MdField::Raw(raw) => self.raw_value(kind, shape, field, raw)?,
        })
    }

    /// A value kiln writes verbatim: a word, a number, or a typed
    /// integer constant (`i32 7`).
    fn raw_value(
        &mut self,
        kind: DiKind,
        shape: Option<DiShape>,
        field: &str,
        raw: &str,
    ) -> R<Val<'k>> {
        let unread = || Fallback::new("debug:raw");
        if let Some((ty, value)) = raw.split_once(' ') {
            let constant = self.typed_int(ty, value).ok_or_else(unread)??;
            return Ok(Val::Node(Some(self.constant_node(constant)?)));
        }
        match raw {
            "true" => return Ok(Val::Bool(true)),
            "false" => return Ok(Val::Bool(false)),
            "null" => return Ok(Val::Node(None)),
            _ => {}
        }
        let first = raw.bytes().next().ok_or_else(unread)?;
        if first == b'-' || first.is_ascii_digit() {
            if shape == Some(DiShape::Node) {
                return Err(unread());
            }
            let (negative, digits) = match raw.strip_prefix('-') {
                Some(digits) => (true, digits),
                None => (false, raw),
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(unread());
            }
            let value = match (negative, parse_u64(digits)) {
                (false, Some(v)) => Some(Val::UInt(v)),
                (true, Some(v)) if v <= 1 << 63 => Some(Val::Int((v as i64).wrapping_neg())),
                _ => None,
            };
            return match value {
                Some(value) => Ok(value),
                None if kind == DiKind::Enumerator => {
                    self.module.record_unsupported(kit::WIDE_ENUMERATOR, None);
                    Ok(Val::UInt(0))
                }
                None => Err(unread()),
            };
        }
        match named_code(shape, field, raw.as_bytes()) {
            Some(code) => Ok(Val::UInt(code)),
            None => Err(unread()),
        }
    }

    /// `iN v` as the reader's `int_literal` reads it; `None` for another
    /// spelling.
    fn typed_int(&mut self, ty: &str, value: &str) -> Option<R<hod_ir::ConstId>> {
        let bits: u32 = ty.strip_prefix('i')?.parse().ok()?;
        let ty = match bits {
            1 => TypeId::I1,
            8 => TypeId::I8,
            16 => TypeId::I16,
            32 => TypeId::I32,
            64 => TypeId::I64,
            _ => match self.module.types.int(bits) {
                Ok(ty) => ty,
                Err(e) => return Some(ir(Err(e))),
            },
        };
        let (consts, types) = (&mut self.module.consts, &self.module.types);
        let made = match value {
            "true" | "false" if ty == TypeId::I1 => {
                consts.int(types, ty, u64::from(value == "true"))
            }
            _ => {
                let (negative, digits) = match value.strip_prefix('-') {
                    Some(digits) => (true, digits),
                    None => (false, value),
                };
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                if digits.len() <= 18 {
                    let v: u64 = digits.parse().ok()?;
                    if negative {
                        consts.int_signed(types, ty, -(v as i64))
                    } else {
                        consts.int(types, ty, v)
                    }
                } else {
                    // The value modulo 2^(64 * words), as the reader keeps it.
                    let magnitude: u128 = digits.parse().ok()?;
                    let v = if negative { magnitude.wrapping_neg() } else { magnitude };
                    let words = (bits as usize).div_ceil(64);
                    if words > 2 {
                        return None;
                    }
                    let all = [v as u64, (v >> 64) as u64];
                    consts.int_from_words(types, ty, &all[..words])
                }
            }
        };
        Some(ir(made))
    }

    /// Defines the numbered `DILocation` `md` as `id` (the reader's
    /// `location_body`).
    fn define_location(&mut self, md: k::MetadataId, id: LocId) -> R<()> {
        let k::MdNode::Specialized { distinct, fields, .. } = self.k.md_node(md) else {
            return Err(Fallback::new("debug:location"));
        };
        let shape = || Fallback::new("debug:location");
        let mut loc = DiLocation { line: 0, column: 0, scope: DiId::NONE, inlined_at: LocId::NONE };
        let mut atom = DiAtom::NONE;
        let mut scope = false;
        for (field, value) in fields {
            match (*field, value) {
                ("line", &k::MdField::UInt(v)) => {
                    loc.line = u32::try_from(v).map_err(|_| shape())?;
                }
                ("column", &k::MdField::UInt(v)) => {
                    loc.column = u16::try_from(v).map_err(|_| shape())?;
                }
                ("scope", &k::MdField::Node(Some(n))) => {
                    scope = true;
                    loc.scope = self.node_ref(n)?.ok_or_else(shape)?;
                }
                ("inlinedAt", k::MdField::Node(None)) => loc.inlined_at = LocId::NONE,
                ("inlinedAt", &k::MdField::Node(Some(n))) => {
                    loc.inlined_at = self.module_loc(n)?.unwrap_or(LocId::NONE);
                }
                ("atomGroup", &k::MdField::UInt(v)) => atom.group = v,
                ("atomRank", &k::MdField::UInt(v)) => {
                    atom = kit::di_atom(atom.group, v).ok_or_else(shape)?;
                }
                (field, _) => match di_field(b"DILocation", field.as_bytes()) {
                    Some(DiMapping::Dropped(_)) => {}
                    _ => return Err(shape()),
                },
            }
        }
        if !scope {
            return Err(shape());
        }
        ir(self.module.debug.define_loc_in_atom(id, loc, atom, *distinct))
    }

    /// The location a numbered node is, at the module level: `None`
    /// (and `debug:invalid` recorded) for one that is not.
    fn module_loc(&mut self, md: k::MetadataId) -> R<Option<LocId>> {
        if !self.k.md_node(md).is_numbered() {
            return Err(Fallback::new("debug:location-ref"));
        }
        if let Handle::Loc(loc) = self.handle(md) {
            return Ok(Some(loc));
        }
        self.module.record_unsupported(kit::DEBUG_INVALID, None);
        Ok(None)
    }

    /// The node a global value's `!dbg` names (the reader's
    /// `global_dbg`).
    fn global_dbg(&self, md: k::MetadataId) -> R<Option<DiId>> {
        match self.k.md_node(md) {
            node if node.is_numbered() => match self.handle(md) {
                Handle::Node(id) => Ok(Some(id)),
                _ => Ok(None),
            },
            k::MdNode::Specialized { name: "DIExpression", .. } => Ok(None),
            _ => Err(Fallback::new("debug:attachment")),
        }
    }

    /// A function's subprogram from the `!dbg` attachments collected
    /// (the reader's `function_dbg`).
    pub(super) fn function_dbg(&mut self) -> R<Option<DiId>> {
        let mut subprogram = None;
        for at in 0..self.dbg.attached.len() {
            let node = self.global_dbg(self.dbg.attached[at])?;
            match kit::function_subprogram(&self.module, node) {
                Some(id) => subprogram = Some(id),
                None => self.note(kit::DEBUG_INVALID),
            }
        }
        Ok(subprogram)
    }

    /// Attaches a variable's global-variable expressions (the reader's
    /// `variable_dbg`).
    pub(super) fn variable_dbg(&mut self, sym: SymId) -> R<()> {
        for at in 0..self.dbg.attached.len() {
            let node = self.global_dbg(self.dbg.attached[at])?;
            if !ir(kit::variable_expression(&mut self.module, sym, node))? {
                self.note(kit::DEBUG_INVALID);
            }
        }
        Ok(())
    }

    // ---- in a body ----

    /// The location a body's `!dbg` names; `None`, with `debug:invalid`
    /// noted against the function, for anything else.
    pub(super) fn body_loc(&mut self, md: k::MetadataId) -> R<Option<LocId>> {
        let node = self.k.md_node(md);
        if !node.is_numbered() && !matches!(node, k::MdNode::Specialized { .. }) {
            return Err(Fallback::new("debug:location-ref"));
        }
        match self.handle(md) {
            Handle::Loc(loc) if node.is_numbered() => Ok(Some(loc)),
            _ => {
                self.note(kit::DEBUG_INVALID);
                Ok(None)
            }
        }
    }

    /// A record's variable or label: a numbered node of the kind named.
    fn record_node(&mut self, md: k::MetadataId, want: &str) -> R<DiId> {
        let node = self.k.md_node(md);
        let found = match node {
            k::MdNode::Specialized { name, .. } => *name,
            k::MdNode::Tuple { .. } => "",
            _ => return Err(Fallback::new("debug:record-operand")),
        };
        if node.is_numbered() {
            if let Handle::Node(id) = self.handle(md)
                && found == want
            {
                return Ok(id);
            }
            self.note(kit::DEBUG_INVALID);
        } else if found == want {
            self.note("debug:node-in-body");
        } else {
            self.note(kit::DEBUG_INVALID);
        }
        Ok(DiId::NONE)
    }

    /// A debug record's variable (or label) and expression, after its
    /// value (the reader's `record_args`).
    pub(super) fn record_head(&mut self, r: &k::DbgRecord) -> R<DebugRecord> {
        let kind = match r.kind {
            k::DbgRecordKind::Declare => DebugRecordKind::Declare,
            k::DbgRecordKind::Value => DebugRecordKind::Value,
            k::DbgRecordKind::Label => DebugRecordKind::Label,
        };
        let mut record = DebugRecord::new(kind);
        if kind == DebugRecordKind::Label {
            record.var = self.record_node(r.var, "DILabel")?;
            return Ok(record);
        }
        record.var = self.record_node(r.var, "DILocalVariable")?;
        record.expr = self.expression(r.expr)?;
        Ok(record)
    }
}
