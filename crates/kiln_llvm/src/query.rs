//! Module-level read-only predicates (`spec/llvm.md` "Queries", WP-1559):
//! questions the driver asks about a finished module without printing it,
//! each answering exactly what a scan of [`Module::to_text`]'s output
//! would.
//!
//! **Cost.** The attribute predicates read the interned attribute table
//! first: when no set holds the attribute, they answer `false` without
//! looking at a function. Otherwise they read each printed function's
//! attribute set, then each function's call records (one per call site
//! ever built, not its instructions); only a call record whose set holds
//! the attribute makes them walk that one function's block layout, to
//! confirm the call is still placed (the printer numbers the group of a
//! placed call only). The declaration predicate reads each function's
//! header. No predicate reads an instruction operand, a constant or
//! metadata.
use crate::attr::Attr;
use crate::inst::InstKind;
use crate::module::Module;
use crate::value::AttrSetId;

impl Module {
    /// Whether a function attribute group the printer writes (`attributes
    /// #N = { ... }`: the function attributes of a printed function,
    /// declaration or definition, or of a placed `call` / `invoke` /
    /// `callbr`) holds an attribute `pred` accepts.
    pub fn has_fn_attr_group(&self, pred: impl Fn(&Attr) -> bool) -> bool {
        let candidates: Vec<bool> = (0..self.attrs.len())
            .map(|i| {
                let id = AttrSetId(u32::try_from(i).unwrap_or(u32::MAX));
                self.attrs.get(id).iter().any(&pred)
            })
            .collect();
        // The empty set holds nothing, so `candidates[0]` is false.
        if !candidates.iter().any(|&c| c) {
            return false;
        }
        let holds = |s: AttrSetId| candidates.get(s.index()).copied().unwrap_or(false);
        if self.ordered_funcs().any(|f| holds(f.attrs.func)) {
            return true;
        }
        self.ordered_funcs().any(|f| {
            f.calls.iter().any(|c| holds(c.attrs.func))
                && f.layout.iter().any(|&bb| {
                    f.blocks[bb.index()].insts.iter().any(|&i| match f.insts[i.index()].kind {
                        InstKind::Call(c) | InstKind::Invoke(c) | InstKind::CallBr(c) => {
                            holds(f.calls[c as usize].attrs.func)
                        }
                        _ => false,
                    })
                })
        })
    }

    /// Whether a printed function attribute group holds the string
    /// attribute with one of the `keys` ([`Attr::Str`], by its key).
    pub fn has_fn_attr_key(&self, keys: &[&str]) -> bool {
        self.has_fn_attr_group(|a| matches!(a, Attr::Str(k, _) if keys.contains(&&**k)))
    }

    /// Whether the module declares (a function without a body, printed
    /// `declare`) a function whose name starts with `prefix` and prints
    /// unquoted (`@llvm.coro.id`, not `@"llvm.coro.x y"`).
    pub fn declares_prefixed(&self, prefix: &str) -> bool {
        self.ordered_funcs().any(|f| {
            f.is_declaration()
                && f.name.starts_with(prefix)
                && !crate::print::name_needs_quotes(&f.name)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Attr, AttrKind, AttrList, CallOpts, IrBuilder, Module, TypeId, Value};

    fn always(a: &Attr) -> bool {
        *a == Attr::Flag(AttrKind::AlwaysInline)
    }

    /// The text scan the predicates must agree with: a group line holding
    /// the keyword.
    fn text_has_always(m: &Module) -> bool {
        m.to_text().lines().any(|l| {
            l.starts_with("attributes #") && l.split([' ', '{', '}']).any(|w| w == "alwaysinline")
        })
    }

    #[test]
    fn interned_but_unused_sets_do_not_count() {
        let mut m = Module::new("t");
        let void = TypeId::VOID;
        let fty = m.types.function(void, &[], false);
        let f = m.add_function("f", fty).unwrap();
        let _unused = m.attr_set(&[Attr::Flag(AttrKind::AlwaysInline)]);
        let mut bld = IrBuilder::new(&mut m, f);
        let bb = bld.append_block("entry");
        bld.position_at_end(bb);
        bld.ret_void().unwrap();
        assert!(!m.has_fn_attr_group(always));
        assert!(!text_has_always(&m));
    }

    #[test]
    fn definition_declaration_and_call_site() {
        let void = TypeId::VOID;
        // On a declaration.
        let mut m = Module::new("t");
        let fty = m.types.function(void, &[], false);
        let g = m.add_function("g", fty).unwrap();
        let set = m.attr_set(&[Attr::Flag(AttrKind::AlwaysInline)]);
        m.func_mut(g).attrs.func = set;
        assert!(m.has_fn_attr_group(always));
        assert!(text_has_always(&m));
        // On a call site only, then that call's block removed.
        let mut m = Module::new("t");
        let fty = m.types.function(void, &[], false);
        let g = m.add_function("g", fty).unwrap();
        let f = m.add_function("f", fty).unwrap();
        let set = m.attr_set(&[Attr::Flag(AttrKind::AlwaysInline)]);
        let opts = CallOpts {
            attrs: AttrList { func: set, ..AttrList::default() },
            ..CallOpts::default()
        };
        let mut bld = IrBuilder::new(&mut m, f);
        let entry = bld.append_block("entry");
        let dead = bld.append_block("dead");
        bld.position_at_end(entry);
        bld.ret_void().unwrap();
        bld.position_at_end(dead);
        bld.call_ex(fty, Value::func(g), &[], &opts).unwrap();
        bld.ret_void().unwrap();
        assert!(m.has_fn_attr_group(always));
        assert!(text_has_always(&m));
        m.func_mut(f).remove_block(dead);
        assert!(!m.has_fn_attr_group(always));
        assert!(!text_has_always(&m));
    }

    #[test]
    fn string_keys_and_declared_prefixes() {
        let void = TypeId::VOID;
        let mut m = Module::new("t");
        let fty = m.types.function(void, &[], false);
        let f = m.add_function("f", fty).unwrap();
        let set = m.attr_set(&[Attr::Str("x".into(), "instrument-function-entry".into())]);
        m.func_mut(f).attrs.func = set;
        assert!(!m.has_fn_attr_key(&["instrument-function-entry"]));
        let set = m.attr_set(&[Attr::Str("instrument-function-entry".into(), "e".into())]);
        m.func_mut(f).attrs.func = set;
        assert!(m.has_fn_attr_key(&["instrument-function-entry"]));
        assert!(!m.declares_prefixed("llvm.coro."));
        m.add_function("llvm.coro.x y", fty).unwrap();
        assert!(!m.declares_prefixed("llvm.coro."), "a quoted name is no intrinsic");
        m.add_function("llvm.coro.id", fty).unwrap();
        assert!(m.declares_prefixed("llvm.coro."));
        // A definition of that name is not a declaration.
        let mut m = Module::new("t");
        let fty = m.types.function(void, &[], false);
        let coro_end = m.add_function("llvm.coro.end", fty).unwrap();
        let mut bld = IrBuilder::new(&mut m, coro_end);
        let bb = bld.append_block("entry");
        bld.position_at_end(bb);
        bld.ret_void().unwrap();
        assert!(!m.declares_prefixed("llvm.coro."));
    }
}
