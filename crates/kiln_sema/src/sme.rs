//! The `AArch64` SME keyword attributes (WP-1426; ACLE §"SME language
//! extensions", clang `handleArmStateAttribute`, `handleArmNewAttr`,
//! `ActOnFunctionDeclarator`, `checkCall`): `__arm_streaming`,
//! `__arm_streaming_compatible`, `__arm_in`, `__arm_out`, `__arm_inout`,
//! `__arm_preserves` and `__arm_agnostic` are function type attributes,
//! consumed into [`FuncInfo::sme`] like a calling convention
//! ([`Sema::apply_cc_attrs`]); `__arm_locally_streaming` and `__arm_new`
//! are declaration attributes of a function. spec/types.md §5.4.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax, SME_KEYWORDS};
use kiln_ast::types::{FuncInfo, sme};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// The keyword of state value `v` (`sme::IN` ...).
fn state_keyword(v: u16) -> &'static str {
    match v {
        sme::IN => "__arm_in",
        sme::OUT => "__arm_out",
        sme::INOUT => "__arm_inout",
        _ => "__arm_preserves",
    }
}

impl Sema<'_, '_> {
    /// The SME keyword attribute `SME_KEYWORDS[idx]` at `loc` with the
    /// string-literal arguments `states`: an [`AttrKind::SME_TYPE`],
    /// [`AttrKind::ARM_LOCALLY_STREAMING`] or [`AttrKind::ARM_NEW`] node,
    /// or `None` after an error (not `AArch64`, a missing or unknown
    /// state).
    pub fn act_on_sme_attribute(
        &mut self,
        idx: usize,
        states: &[ExprId],
        loc: Loc,
    ) -> Option<AttrId> {
        let name = SME_KEYWORDS.get(idx).copied()?;
        if self.abi.triple.arch != kiln_abi::target::Arch::AArch64 {
            let msg = self
                .diag(DiagId::sema_keyword_not_supported_on_target, loc)
                .arg(format!("'{name}'"));
            self.emit(msg);
            return None;
        }
        let takes_states = idx >= 2 && idx != 7;
        if takes_states && states.is_empty() {
            let msg = self.diag(DiagId::sema_arm_missing_state, loc).arg(format!("'{name}'"));
            self.emit(msg);
            return None;
        }
        let (mut za, mut zt0) = (None, None);
        for &e in states {
            let at = self.ast.exprs.get(e).loc();
            let text = self.string_lit_of(e).map_or_else(String::new, |l| {
                let lit = self.ast.exprs.string_lit(l);
                String::from_utf8_lossy(self.ast.strings.bytes(lit)).into_owned()
            });
            match text.as_str() {
                "za" if idx != 6 => za = Some(at),
                "zt0" if idx != 6 => zt0 = Some(at),
                "sme_za_state" if idx == 6 => za = Some(at),
                _ => {
                    let msg = self.diag(DiagId::sema_arm_unknown_state, at).arg(text);
                    self.emit(msg);
                    return None;
                }
            }
        }
        let sm = self.diags.sm();
        let delta = |at: Option<Loc>| {
            at.and_then(|a| sm.same_entry_distance(loc, a)).filter(|&d| d < 1 << 14).unwrap_or(0)
        };
        #[allow(clippy::cast_possible_truncation, reason = "idx < 9")]
        let b = idx as u32 | delta(za) << 4 | delta(zt0) << 18;
        let (kind, a) = match idx {
            0 => (AttrKind::SME_TYPE, sme::STREAMING),
            1 => (AttrKind::SME_TYPE, sme::STREAMING_COMPATIBLE),
            6 => (AttrKind::SME_TYPE, sme::AGNOSTIC_ZA),
            7 => (AttrKind::ARM_LOCALLY_STREAMING, 0),
            8 => (AttrKind::ARM_NEW, u16::from(za.is_some()) | u16::from(zt0.is_some()) << 1),
            _ => {
                #[allow(clippy::cast_possible_truncation, reason = "idx in 2..=5")]
                let v = idx as u16 - 1;
                let za = if za.is_some() { v << sme::ZA_SHIFT } else { 0 };
                let zt0 = if zt0.is_some() { v << sme::ZT0_SHIFT } else { 0 };
                (AttrKind::SME_TYPE, za | zt0)
            }
        };
        let node = AttrNode { kind, syntax: AttrSyntax::Keyword, flags: 0, loc, a: a.into(), b };
        Some(self.ast.attrs.push(node))
    }

    /// Drops (with `cannot be applied to a declaration`) the SME type
    /// attributes among `nodes` not written in a type position (clang
    /// `ParsedAttr::isTypeAttr` diagnosed in `ProcessDeclAttribute`).
    pub(crate) fn drop_decl_position_sme(&mut self, nodes: &mut Vec<AttrNode>) {
        nodes.retain(|n| {
            if n.kind != AttrKind::SME_TYPE || n.flags & AttrNode::SME_IN_TYPE != 0 {
                return true;
            }
            let msg =
                self.diag(DiagId::sema_keyword_attr_on_decl, n.loc).arg(Self::sme_keyword_shown(n));
            self.emit(msg);
            false
        });
    }

    /// Marks the SME type attributes among `attrs` as written in a type
    /// position (after a declarator-id; [`AttrNode::SME_IN_TYPE`]).
    pub fn mark_sme_in_type(&mut self, attrs: &[AttrId]) {
        for &a in attrs {
            let n = *self.ast.attrs.get(a);
            if n.kind == AttrKind::SME_TYPE {
                self.ast.attrs.set_flags(a, n.flags | AttrNode::SME_IN_TYPE);
            }
        }
    }

    /// The keyword of an SME attribute node, quoted.
    pub(crate) fn sme_keyword_shown(n: &AttrNode) -> String {
        let idx = match n.kind {
            AttrKind::ARM_LOCALLY_STREAMING => 7,
            AttrKind::ARM_NEW => 8,
            _ => (n.b & 15) as usize,
        };
        format!("'{}'", SME_KEYWORDS.get(idx).copied().unwrap_or("__arm_streaming"))
    }

    /// The location of the `"za"` (`zt0` false) or `"zt0"` string of an
    /// SME attribute node, or its keyword's.
    fn sme_state_loc(n: &AttrNode, zt0: bool) -> Loc {
        let d = if zt0 { n.b >> 18 } else { (n.b >> 4) & 0x3fff };
        n.loc.with_offset(d)
    }

    /// The SME bits of `info` with those of the [`AttrKind::SME_TYPE`]
    /// nodes `nodes` added (clang `handleArmStateAttribute` and
    /// `checkMutualExclusion`): a second streaming mode is
    /// `'X' and 'Y' are not compatible` at the first, and a second state
    /// for ZA or ZT0 `conflicting attributes for state`; either is dropped.
    pub(crate) fn merge_sme_attrs(&mut self, info: FuncInfo, nodes: &[AttrNode]) -> u16 {
        let modes = sme::STREAMING | sme::STREAMING_COMPATIBLE;
        let mut bits = info.sme();
        let mut first_mode: Option<AttrNode> = None;
        'nodes: for n in nodes.iter().filter(|n| n.kind == AttrKind::SME_TYPE) {
            #[allow(clippy::cast_possible_truncation, reason = "SME bits")]
            let add = n.a as u16;
            if add & modes != 0 {
                if let Some(f) = first_mode
                    && f.a != n.a
                {
                    let msg = self
                        .diag(DiagId::sema_keyword_attrs_not_compatible, f.loc)
                        .arg(Self::sme_keyword_shown(n))
                        .arg(Self::sme_keyword_shown(&f));
                    let note = self.diag(DiagId::note_conflicting_attribute, n.loc);
                    self.emit(msg.with_note(note));
                    continue;
                }
                first_mode.get_or_insert(*n);
            }
            for (shift, zt0, what) in [(sme::ZA_SHIFT, false, "za"), (sme::ZT0_SHIFT, true, "zt0")]
            {
                let new = (add >> shift) & 7;
                let old = (bits >> shift) & 7;
                if new != 0 && old != 0 && new != old {
                    let msg = self
                        .diag(DiagId::sema_arm_conflicting_state, Self::sme_state_loc(n, zt0))
                        .arg(what);
                    self.emit(msg);
                    continue 'nodes;
                }
            }
            bits |= add;
        }
        bits
    }

    /// Whether function `f` runs in streaming mode (clang
    /// `IsArmStreamingFunction` with `IncludeLocallyStreaming`).
    pub(crate) fn is_streaming_fn(&self, f: DeclId) -> bool {
        self.find_attr(f, AttrKind::ARM_LOCALLY_STREAMING).is_some()
            || self.fn_sme_bits(f) & sme::STREAMING != 0
    }

    /// The SME bits of function `f`'s type.
    fn fn_sme_bits(&self, f: DeclId) -> u16 {
        self.ast.decls.get(f).ty().and_then(|t| self.fn_info_through(t)).map_or(0, FuncInfo::sme)
    }

    /// The ZA (bit 0) and ZT0 (bit 1) of `__arm_new` on function `f`.
    fn arm_new_of(&self, f: DeclId) -> u32 {
        self.find_attr(f, AttrKind::ARM_NEW).map_or(0, |a| self.ast.attrs.get(a).a)
    }

    /// `__arm_locally_streaming` / `__arm_new` on declaration `d`
    /// (clang `handleArmNewAttr`): only on a function, and `__arm_new`
    /// not for a state the function's type already has.
    pub(crate) fn check_sme_decl_attr(&mut self, d: DeclId, node: AttrNode) -> bool {
        if !self.ast.decls.get(d).kind().is_function() {
            let msg = self
                .diag(DiagId::sema_keyword_wrong_decl_type, node.loc)
                .arg(Self::sme_keyword_shown(&node))
                .arg("functions");
            self.emit(msg);
            return false;
        }
        if node.kind != AttrKind::ARM_NEW {
            return true;
        }
        let bits = self.fn_sme_bits(d);
        let mut ok = true;
        for (bit, state, what) in [(1, sme::za(bits), "za"), (2, sme::zt0(bits), "zt0")] {
            if node.a & bit != 0 && state != 0 {
                let msg = self
                    .diag(DiagId::sema_keyword_attrs_not_compatible, node.loc)
                    .arg(format!("'__arm_new(\"{what}\")'"))
                    .arg(format!("'{}(\"{what}\")'", state_keyword(state)));
                self.emit(msg);
                ok = false;
            }
        }
        ok
    }

    /// clang `MergeFunctionDecl`: a redeclaration with other SME bits than
    /// the previous declaration's.
    pub(crate) fn check_sme_redecl(&mut self, ty: QualType, pty: QualType, prev: DeclId, at: Loc) {
        let (Some(new), Some(old)) = (self.fn_info_through(ty), self.fn_info_through(pty)) else {
            return;
        };
        if new.sme() == old.sme() {
            return;
        }
        let msg = self
            .diag(DiagId::sema_sme_attr_mismatch, at)
            .arg(format!("'{}'", self.ty_str(ty)))
            .arg(format!("'{}'", self.ty_str(pty)));
        let msg = self.note_prev(msg, prev, false);
        self.emit(msg);
    }

    /// clang `CheckOverridingFunctionAttributes`: an overrider has the SME
    /// attributes of the function it overrides. True if it differs.
    pub(crate) fn check_override_sme(
        &mut self,
        name: String,
        at: Loc,
        ty: QualType,
        o: DeclId,
    ) -> bool {
        let oty = self.ast.decls.get(o).ty().unwrap_or(QualType::ERROR);
        let sme = |s: &Self, t: QualType| s.fn_info_through(t).map_or(0, FuncInfo::sme);
        if sme(self, ty) == sme(self, oty) {
            return false;
        }
        let msg = self
            .diag(DiagId::sema_conflicting_overriding_attributes, at)
            .arg(name)
            .arg(format!("'{}'", self.ty_str(ty)))
            .arg(format!("'{}'", self.ty_str(oty)))
            .with_note(self.overridden_note(o));
        self.emit(msg);
        true
    }

    /// clang `ActOnFunctionDeclarator` for a definition: a streaming
    /// function needs `sme`, ZA state `sme` and ZT0 state `sme2`.
    pub(crate) fn check_sme_definition(&mut self, f: DeclId) {
        let bits = self.fn_sme_bits(f);
        let new = self.arm_new_of(f);
        let streaming = self.is_streaming_fn(f);
        if bits == 0 && new == 0 && !streaming {
            return;
        }
        let at = self.ast.decls.get(f).loc();
        let has_sme = self.fn_feature(Some(f), "sme", self.module_feature("sme"));
        let has_v2 = self.fn_feature(Some(f), "sme2", self.module_feature("sme2"));
        if streaming && !has_sme {
            self.error(DiagId::sema_sme_definition_sm_no_sme, at);
        }
        if (sme::za(bits) != 0 || new & 1 != 0) && !has_sme {
            self.error(DiagId::sema_sme_definition_za_no_sme, at);
        }
        if (sme::zt0(bits) != 0 || new & 2 != 0) && !has_v2 {
            self.error(DiagId::sema_sme_definition_zt0_no_sme2, at);
        }
    }

    /// clang `checkCall` for a callee of type `info` at `rparen`: a
    /// streaming callee needs `sme`; a callee sharing ZA or ZT0 a caller
    /// with that state; a callee sharing ZT0 but not ZA from a caller
    /// with ZA state would need a ZA spill.
    pub(crate) fn check_sme_call(&mut self, info: FuncInfo, rparen: Loc) {
        let want = info.sme();
        if want == 0 {
            return;
        }
        let cur = Some(self.ctx).filter(|&c| self.ast.decls.get(c).kind().is_function());
        if want & sme::STREAMING != 0 && !self.fn_feature(cur, "sme", self.module_feature("sme")) {
            self.error(DiagId::sema_sme_call_no_sme, rparen);
        }
        let (bits, new) = cur.map_or((0, 0), |c| (self.fn_sme_bits(c), self.arm_new_of(c)));
        let has_za = new & 1 != 0 || sme::za(bits) != 0;
        let has_t0 = new & 2 != 0 || sme::zt0(bits) != 0;
        let (shares_za, shares_t0) = (sme::za(want) != 0, sme::zt0(want) != 0);
        if shares_za && !has_za {
            self.error(DiagId::sema_sme_za_call_no_za_state, rparen);
        }
        if shares_t0 && !has_t0 {
            self.error(DiagId::sema_sme_zt0_call_no_zt0_state, rparen);
        }
        if has_za && shares_t0 && !shares_za {
            let note = self.diag(DiagId::note_sme_use_preserves_za, rparen);
            let msg = self.diag(DiagId::sema_sme_unimplemented_za_save_restore, rparen);
            self.emit(msg.with_note(note));
        }
    }
}
