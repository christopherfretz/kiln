//! Loop metadata (clang `CGLoopInfo`; spec/codegen.md "Loop metadata"):
//! the `LoopAttributes` of a loop from its loop-hint attributes
//! (`LoopInfoStack::push`) and the `llvm.loop` node built from them when
//! the loop is finished (`LoopInfo::finish` and the `create*Metadata`
//! chain: full unroll, distribute, vectorize, unroll-and-jam, partial
//! unroll, pipelining, each handing its followup loop's properties on).

use kiln_ast::StmtId;
use kiln_ast::attr::{AttrKind, LoopHint, LoopHintOption as O, LoopHintState as St};
use kiln_ast::ids::RawId;
use kiln_llvm::{MetadataId, Module, TypeId as LTy};

use crate::fir::Blk;
use crate::module::Cg;

/// clang `LoopAttributes::LVEnableState`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LState {
    #[default]
    Unspecified,
    Enable,
    Disable,
    Full,
}

/// clang `LoopAttributes`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LoopAttrs {
    vectorize_enable: LState,
    unroll_enable: LState,
    unroll_and_jam_enable: LState,
    vectorize_predicate_enable: LState,
    vectorize_width: u32,
    vectorize_scalable: LState,
    interleave_count: u32,
    unroll_count: u32,
    unroll_and_jam_count: u32,
    distribute_enable: LState,
    pipeline_disabled: bool,
    pipeline_ii: u32,
    must_progress: bool,
    /// `IsParallel` (`vectorize(assume_safety)`, `interleave(assume_safety)`).
    is_parallel: bool,
    /// `[[clang::code_align(N)]]` (`CodeAlign`).
    code_align: u32,
}

impl LoopAttrs {
    /// clang's `LoopInfo` constructor: whether the loop gets an id.
    fn any(&self) -> bool {
        self.is_parallel
            || self.vectorize_width != 0
            || self.vectorize_scalable != LState::Unspecified
            || self.interleave_count != 0
            || self.unroll_count != 0
            || self.unroll_and_jam_count != 0
            || self.pipeline_disabled
            || self.pipeline_ii != 0
            || self.vectorize_predicate_enable != LState::Unspecified
            || self.vectorize_enable != LState::Unspecified
            || self.unroll_enable != LState::Unspecified
            || self.unroll_and_jam_enable != LState::Unspecified
            || self.distribute_enable != LState::Unspecified
            || self.must_progress
            || self.code_align != 0
    }
}

/// An active loop (clang `LoopInfo`).
#[derive(Clone, Debug)]
pub(crate) struct LoopFrame {
    pub header: Blk,
    /// The loop id, a `distinct` placeholder until the loop is finished
    /// (clang `TempLoopID`).
    pub md: Option<MetadataId>,
    attrs: LoopAttrs,
    /// clang `UnrollAndJamInnerFollowup`.
    inner_followup: Option<Vec<MetadataId>>,
    /// clang `AccessGroup`: the `distinct !{}` of a parallel loop.
    access_group: Option<MetadataId>,
}

/// The `create*Metadata` chain of one loop.
struct MdBuilder<'a> {
    m: &'a mut Module,
    inner_followup: Option<&'a [MetadataId]>,
    access_group: Option<MetadataId>,
}

type Props = Vec<MetadataId>;

impl MdBuilder<'_> {
    /// `!{!"name"}`.
    fn flag(&mut self, name: &str) -> MetadataId {
        let s = self.m.md_string(name.as_bytes());
        self.m.md_tuple(&[Some(s)])
    }

    /// `!{!"name", iN v}`.
    fn value(&mut self, name: &str, ty: LTy, v: u32) -> MetadataId {
        let s = self.m.md_string(name.as_bytes());
        let c = self.m.const_int(ty, i128::from(v));
        let c = self.m.md_const(c).expect("a constant");
        self.m.md_tuple(&[Some(s), Some(c)])
    }

    fn followup(&mut self, name: &str, props: &[MetadataId]) -> MetadataId {
        let s = self.m.md_string(name.as_bytes());
        let ops: Vec<_> = std::iter::once(Some(s)).chain(props.iter().copied().map(Some)).collect();
        self.m.md_tuple(&ops)
    }

    fn pipelining(&mut self, a: &LoopAttrs, mut props: Props, has: &mut bool) -> Props {
        let enabled = if a.pipeline_disabled {
            Some(false)
        } else if a.pipeline_ii != 0 {
            Some(true)
        } else {
            None
        };
        if enabled != Some(true) {
            if enabled == Some(false) {
                props.push(self.value("llvm.loop.pipeline.disable", LTy::I1, 1));
            }
            return props;
        }
        if a.pipeline_ii > 0 {
            props.push(self.value(
                "llvm.loop.pipeline.initiationinterval",
                LTy::I32,
                a.pipeline_ii,
            ));
        }
        *has = true;
        props
    }

    fn partial_unroll(&mut self, a: &LoopAttrs, props: Props, has: &mut bool) -> Props {
        let enabled = match a.unroll_enable {
            LState::Disable => Some(false),
            LState::Full => None,
            LState::Unspecified if a.unroll_count == 0 => None,
            _ => Some(true),
        };
        if enabled != Some(true) {
            return self.pipelining(a, props, has);
        }
        let mut fprops = props.clone();
        fprops.push(self.flag("llvm.loop.unroll.disable"));
        let mut fhas = false;
        let followup = self.pipelining(a, fprops, &mut fhas);
        let mut args = props;
        if a.unroll_count > 0 {
            args.push(self.value("llvm.loop.unroll.count", LTy::I32, a.unroll_count));
        }
        if a.unroll_enable == LState::Enable {
            args.push(self.flag("llvm.loop.unroll.enable"));
        }
        if fhas {
            args.push(self.followup("llvm.loop.unroll.followup_all", &followup));
        }
        *has = true;
        args
    }

    fn unroll_and_jam(&mut self, a: &LoopAttrs, mut props: Props, has: &mut bool) -> Props {
        let enabled = match a.unroll_and_jam_enable {
            LState::Disable => Some(false),
            LState::Enable => Some(true),
            _ if a.unroll_and_jam_count != 0 => Some(true),
            _ => None,
        };
        if enabled != Some(true) {
            if enabled == Some(false) {
                props.push(self.flag("llvm.loop.unroll_and_jam.disable"));
            }
            return self.partial_unroll(a, props, has);
        }
        let mut fprops = props.clone();
        fprops.push(self.flag("llvm.loop.unroll_and_jam.disable"));
        let mut fhas = false;
        let followup = self.partial_unroll(a, fprops, &mut fhas);
        let mut args = props;
        if a.unroll_and_jam_count > 0 {
            let v = self.value("llvm.loop.unroll_and_jam.count", LTy::I32, a.unroll_and_jam_count);
            args.push(v);
        }
        if a.unroll_and_jam_enable == LState::Enable {
            args.push(self.flag("llvm.loop.unroll_and_jam.enable"));
        }
        if fhas {
            args.push(self.followup("llvm.loop.unroll_and_jam.followup_outer", &followup));
        }
        if let Some(inner) = self.inner_followup {
            args.push(self.followup("llvm.loop.unroll_and_jam.followup_inner", inner));
        }
        *has = true;
        args
    }

    fn vectorize(&mut self, a: &LoopAttrs, mut props: Props, has: &mut bool) -> Props {
        let enabled = if a.vectorize_enable == LState::Disable {
            Some(false)
        } else if a.vectorize_enable != LState::Unspecified
            || a.vectorize_predicate_enable != LState::Unspecified
            || a.interleave_count != 0
            || a.vectorize_width != 0
            || a.vectorize_scalable != LState::Unspecified
        {
            Some(true)
        } else {
            None
        };
        if enabled != Some(true) {
            if enabled == Some(false) {
                props.push(self.value("llvm.loop.vectorize.enable", LTy::I1, 0));
            }
            return self.unroll_and_jam(a, props, has);
        }
        let mut args = props.clone();
        let mut predicate = false;
        if a.vectorize_predicate_enable != LState::Unspecified {
            predicate = a.vectorize_predicate_enable == LState::Enable;
            let v =
                self.value("llvm.loop.vectorize.predicate.enable", LTy::I1, u32::from(predicate));
            args.push(v);
        }
        if a.vectorize_width > 0 {
            args.push(self.value("llvm.loop.vectorize.width", LTy::I32, a.vectorize_width));
        }
        if a.vectorize_scalable != LState::Unspecified {
            let scalable = a.vectorize_scalable == LState::Enable;
            let v = self.value("llvm.loop.vectorize.scalable.enable", LTy::I1, u32::from(scalable));
            args.push(v);
        }
        if a.interleave_count > 0 {
            args.push(self.value("llvm.loop.interleave.count", LTy::I32, a.interleave_count));
        }
        let mut vectorize = false;
        if a.vectorize_enable != LState::Unspecified
            || (predicate && a.vectorize_width != 1)
            || a.vectorize_width > 1
            || a.vectorize_scalable == LState::Enable
            || (a.vectorize_scalable == LState::Disable && a.vectorize_width != 1)
        {
            vectorize = a.vectorize_enable != LState::Disable;
            args.push(self.value("llvm.loop.vectorize.enable", LTy::I1, u32::from(vectorize)));
        }
        let mut fprops = if vectorize { props } else { Vec::new() };
        fprops.push(self.flag("llvm.loop.isvectorized"));
        let mut fhas = false;
        let followup = self.unroll_and_jam(a, fprops, &mut fhas);
        if fhas {
            if vectorize {
                args.push(self.followup("llvm.loop.vectorize.followup_all", &followup));
            } else {
                args.extend(followup);
            }
        }
        *has = true;
        args
    }

    fn distribute(&mut self, a: &LoopAttrs, mut props: Props, has: &mut bool) -> Props {
        let enabled = match a.distribute_enable {
            LState::Disable => Some(false),
            LState::Enable => Some(true),
            _ => None,
        };
        if enabled != Some(true) {
            if enabled == Some(false) {
                props.push(self.value("llvm.loop.distribute.enable", LTy::I1, 0));
            }
            return self.vectorize(a, props, has);
        }
        let mut fhas = false;
        let followup = self.vectorize(a, props.clone(), &mut fhas);
        let mut args = props;
        args.push(self.value("llvm.loop.distribute.enable", LTy::I1, 1));
        if fhas {
            args.push(self.followup("llvm.loop.distribute.followup_all", &followup));
        }
        *has = true;
        args
    }

    fn full_unroll(&mut self, a: &LoopAttrs, mut props: Props, has: &mut bool) -> Props {
        match a.unroll_enable {
            LState::Full => {
                props.push(self.flag("llvm.loop.unroll.full"));
                *has = true;
                props
            }
            s => {
                if s == LState::Disable {
                    props.push(self.flag("llvm.loop.unroll.disable"));
                }
                self.distribute(a, props, has)
            }
        }
    }

    /// clang `LoopInfo::createMetadata`.
    fn create(&mut self, a: &LoopAttrs, additional: Props, has: &mut bool) -> Props {
        let mut props = Vec::new();
        if a.must_progress {
            props.push(self.flag("llvm.loop.mustprogress"));
        }
        if a.is_parallel
            && let Some(g) = self.access_group
        {
            let s = self.m.md_string(b"llvm.loop.parallel_accesses");
            props.push(self.m.md_tuple(&[Some(s), Some(g)]));
        }
        if a.code_align > 0 {
            props.push(self.value("llvm.loop.align", LTy::I32, a.code_align));
        }
        props.extend(additional);
        self.full_unroll(a, props, has)
    }
}

impl Cg<'_> {
    /// clang `LoopInfoStack::push`: the attributes of a loop with the
    /// loop hints of `AttributedStmt` `hints` (if any), made active.
    pub(crate) fn push_loop(&mut self, header: Blk, hints: Option<StmtId>, must_progress: bool) {
        let mut a = LoopAttrs::default();
        let ast = self.ast;
        let list = hints.map(|s| ast.stmts.stmt_attrs(s));
        for at in list.into_iter().flat_map(|l| ast.lists.iter::<kiln_ast::AttrId>(l)) {
            let node = *ast.attrs.get(at);
            if node.kind == AttrKind::CODE_ALIGN {
                a.code_align = node.a;
                continue;
            }
            if node.kind != AttrKind::LOOP_HINT {
                continue;
            }
            let h = LoopHint::unpack(node.b);
            let value = kiln_ast::ExprId::from_raw(node.a);
            let v = value.and_then(|e| self.fold_int(e)).unwrap_or(1);
            let v = u32::try_from(v).unwrap_or(0);
            match (h.state, h.option) {
                (St::Disable, O::Vectorize) => {
                    a.vectorize_width = 1;
                    a.vectorize_scalable = LState::Unspecified;
                }
                (St::Disable, O::Interleave) => a.interleave_count = 1,
                (St::Disable, O::Unroll) => a.unroll_enable = LState::Disable,
                (St::Disable, O::UnrollAndJam) => a.unroll_and_jam_enable = LState::Disable,
                (St::Disable, O::VectorizePredicate) => {
                    a.vectorize_predicate_enable = LState::Disable;
                }
                (St::Disable, O::Distribute) => a.distribute_enable = LState::Disable,
                (St::Disable, O::PipelineDisabled) => a.pipeline_disabled = true,
                (St::Enable, O::Vectorize | O::Interleave) => a.vectorize_enable = LState::Enable,
                // The loop is parallel: its memory accesses get its access
                // group, and its id `llvm.loop.parallel_accesses`.
                (St::AssumeSafety, O::Vectorize | O::Interleave) => {
                    a.is_parallel = true;
                    a.vectorize_enable = LState::Enable;
                }
                (St::Enable, O::Unroll) => a.unroll_enable = LState::Enable,
                (St::Enable, O::UnrollAndJam) => a.unroll_and_jam_enable = LState::Enable,
                (St::Enable, O::VectorizePredicate) => {
                    a.vectorize_predicate_enable = LState::Enable;
                }
                (St::Enable, O::Distribute) => a.distribute_enable = LState::Enable,
                (St::Full, O::Unroll) => a.unroll_enable = LState::Full,
                (St::Full, O::UnrollAndJam) => a.unroll_and_jam_enable = LState::Full,
                (St::FixedWidth | St::ScalableWidth, O::VectorizeWidth) => {
                    a.vectorize_scalable =
                        if h.state == St::ScalableWidth { LState::Enable } else { LState::Disable };
                    if value.is_some() {
                        a.vectorize_width = v;
                    }
                }
                (St::Numeric, O::InterleaveCount) => a.interleave_count = v,
                (St::Numeric, O::UnrollCount) => a.unroll_count = v,
                (St::Numeric, O::UnrollAndJamCount) => a.unroll_and_jam_count = v,
                (St::Numeric, O::PipelineInitiationInterval) => a.pipeline_ii = v,
                _ => {}
            }
        }
        a.must_progress = must_progress;
        // At `-O1` (clang `OptimizationLevel > 0` without `UnrollLoops`,
        // which `-O2` and above set) a loop without an unroll hint is not
        // unrolled.
        if self.opts.opt_level == 1 && a.unroll_enable == LState::Unspecified && a.unroll_count == 0
        {
            a.unroll_enable = LState::Disable;
        }
        let access_group = a.is_parallel.then(|| self.m.md_distinct_tuple(&[]));
        let md = a.any().then(|| self.m.md_distinct_tuple(&[None]));
        self.f.loops.push(LoopFrame { header, md, attrs: a, inner_followup: None, access_group });
        if access_group.is_some() {
            self.update_access_group();
        }
    }

    /// The access group of the instructions emitted under the active
    /// loops (clang `LoopInfoStack::InsertHelper`): none, the one
    /// parallel loop's, or the list of every parallel loop's, outermost
    /// first.
    fn update_access_group(&mut self) {
        let groups: Vec<_> = self.f.loops.iter().filter_map(|l| l.access_group).collect();
        let g = match groups.as_slice() {
            [] => None,
            [g] => Some(*g),
            _ => {
                let ops: Vec<_> = groups.into_iter().map(Some).collect();
                Some(self.m.md_tuple(&ops))
            }
        };
        let k = self.m.md_kind("llvm.access.group");
        self.f.fir.set_access_group(k, g);
    }

    /// clang `LoopInfoStack::pop` (`LoopInfo::finish`): the loop's id
    /// gets its properties; under an unroll-and-jam loop they are the
    /// properties before the jam, and the first such inner loop gives
    /// the outer one its `followup_inner`.
    pub(crate) fn pop_loop(&mut self) {
        let l = self.f.loops.pop().expect("an active loop");
        if l.access_group.is_some() {
            self.update_access_group();
        }
        let Some(id) = l.md else { return };
        let mut cur = l.attrs;
        let compat19 = self.opts.clang_compat < 22;
        let mut b = MdBuilder {
            m: &mut self.m,
            inner_followup: l.inner_followup.as_deref(),
            access_group: l.access_group,
        };
        if let Some(p) = self.f.loops.last_mut()
            && (p.attrs.unroll_and_jam_enable != LState::Unspecified
                || p.attrs.unroll_and_jam_count != 0)
        {
            let a = l.attrs;
            let mut before = LoopAttrs {
                vectorize_width: a.vectorize_width,
                vectorize_scalable: a.vectorize_scalable,
                interleave_count: a.interleave_count,
                vectorize_enable: a.vectorize_enable,
                distribute_enable: a.distribute_enable,
                vectorize_predicate_enable: a.vectorize_predicate_enable,
                unroll_and_jam_count: a.unroll_and_jam_count,
                unroll_and_jam_enable: a.unroll_and_jam_enable,
                is_parallel: a.is_parallel,
                ..LoopAttrs::default()
            };
            let mut after = LoopAttrs {
                vectorize_predicate_enable: a.vectorize_predicate_enable,
                unroll_count: a.unroll_count,
                pipeline_disabled: a.pipeline_disabled,
                pipeline_ii: a.pipeline_ii,
                is_parallel: a.is_parallel,
                ..LoopAttrs::default()
            };
            match a.unroll_enable {
                LState::Unspecified | LState::Disable => {
                    before.unroll_enable = a.unroll_enable;
                    after.unroll_enable = a.unroll_enable;
                }
                LState::Full => before.unroll_enable = LState::Full,
                LState::Enable => after.unroll_enable = LState::Enable,
            }
            if p.inner_followup.is_none() {
                let mut before_props = Vec::new();
                if before.vectorize_enable != LState::Unspecified
                    || before.vectorize_predicate_enable != LState::Unspecified
                    || before.interleave_count != 0
                    || before.vectorize_width != 0
                    || before.vectorize_scalable == LState::Enable
                {
                    before_props.push(b.flag("llvm.loop.isvectorized"));
                }
                let mut has = false;
                let inner = b.create(&after, before_props, &mut has);
                if has {
                    // clang 19 made the after-jam properties a distinct
                    // self-referential node of their own (its
                    // `UnrollAndJamInnerFollowup` was a loop id); clang 22
                    // lists them in `followup_inner` itself.
                    p.inner_followup = Some(if compat19 {
                        let n = b.m.md_distinct_tuple(&[None]);
                        let ops: Vec<_> =
                            std::iter::once(Some(n)).chain(inner.into_iter().map(Some)).collect();
                        b.m.md_set_operands(n, &ops).expect("distinct followup node");
                        vec![n]
                    } else {
                        inner
                    });
                }
            }
            cur = before;
        }
        let mut has = false;
        let props = b.create(&cur, Vec::new(), &mut has);
        let ops: Vec<_> = std::iter::once(Some(id)).chain(props.into_iter().map(Some)).collect();
        self.m.md_set_operands(id, &ops).expect("distinct loop id");
    }
}
