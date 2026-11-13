//! `alignas` operands of a pattern instantiated while an image is read
//! (WP-1369, spec/modules.md §11.11, "Forms carried by WP-1369").
//!
//! The importer attaches a declaration's attributes with its record but
//! fills their operand expressions in only once the bodies are built
//! (`image_attr_exprs`): an operand may name any declaration of the image.
//! A class specialization the unit completed is completed with its record
//! (`image_complete_spec`), which instantiates the pattern's attributes
//! ([dcl.align]/1, clang `InstantiateAttrs`); an `alignas(alignof(T))` of
//! the pattern, or of a data member or member class it instantiates, has
//! no operand yet. Its instantiation is then a placeholder `aligned`
//! attribute, with what substituting the operand needs (the arguments and
//! the pattern/specialization pairs); once the operands are in, each is
//! substituted as the instantiation would have, and the record layouts
//! computed in between are dropped.

use kiln_ast::attr::AttrNode;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::fx::FxHashSet;

use crate::Sema;
use crate::templ::args::MultiLevelArgs;
use crate::templ::subst::Substituter;

/// One instantiated `alignas` waiting for its pattern's operand.
#[derive(Debug)]
struct Fix {
    placeholder: AttrId,
    pattern: AttrId,
    args: MultiLevelArgs,
    entity: Option<DeclId>,
    pairs: Vec<(DeclId, DeclId)>,
}

/// The `alignas` attributes whose operand is not in yet, and the
/// instantiations of them made meanwhile.
#[derive(Debug, Default)]
pub(crate) struct AlignedFixups {
    pending: FxHashSet<AttrId>,
    fixes: Vec<Fix>,
}

impl Sema<'_, '_> {
    /// Notes that attribute `a`, an `alignas` with an operand expression,
    /// gets its operand only after the bodies.
    pub(super) fn image_aligned_pending(&mut self, a: AttrId) {
        self.image_texprs.aligned.pending.insert(a);
    }

    /// Substitutes the operands of the `alignas` attributes instantiated
    /// before their pattern's operand was in, now that it is; drops the
    /// layouts computed meanwhile.
    pub(super) fn image_fix_aligned(&mut self) {
        let fixes = std::mem::take(&mut self.image_texprs.aligned.fixes);
        self.image_texprs.aligned.pending.clear();
        if fixes.is_empty() {
            return;
        }
        for f in fixes {
            let node = *self.ast.attrs.get(f.pattern);
            let Some(e) = ExprId::from_raw(node.a) else { continue };
            let mut sub = Substituter::new(self, &f.args);
            sub.entity = f.entity;
            sub.pairs = f.pairs;
            // A pack (`alignas(Ns...)`) expands to its strictest element
            // ([dcl.align]/5); one element is the usual case.
            let Ok(es) = sub.expand_exprs(&[e]) else { continue };
            let mut best: Option<(ExprId, i128)> = None;
            for e in es {
                let Some(e) = self.check_alignment_expr(e, node.loc) else { continue };
                let at = self.begin_loc(e);
                let v = self.eval_int(e, EvalContext::new(EvalKind::Other, at)).unwrap_or(0);
                if best.is_none_or(|(_, b)| v > b) {
                    best = Some((e, v));
                }
            }
            if let Some((e, _)) = best {
                self.ast.attrs.set_args(f.placeholder, e.raw(), 0);
            }
        }
        self.layout_cache = kiln_abi::LayoutCache::default();
    }
}

impl Substituter<'_, '_, '_> {
    /// The instantiation of pattern attribute `id` (`node`) when it is an
    /// `alignas` whose operand is not in yet: a placeholder, substituted
    /// by [`Sema::image_fix_aligned`].
    pub(crate) fn image_aligned_placeholder(
        &mut self,
        id: AttrId,
        node: AttrNode,
    ) -> Option<AttrId> {
        if !self.sema.image_texprs.aligned.pending.contains(&id) {
            return None;
        }
        let placeholder = self.sema.ast.attrs.push(AttrNode { a: 0, b: 0, ..node });
        let fix = Fix {
            placeholder,
            pattern: id,
            args: self.args.clone(),
            entity: self.entity,
            pairs: self.pairs.clone(),
        };
        self.sema.image_texprs.aligned.fixes.push(fix);
        Some(placeholder)
    }
}
