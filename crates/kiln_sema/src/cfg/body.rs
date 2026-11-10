//! The graph of the body being finished, built at its first query
//! (spec/sema.md §7.5.1). The end of a function body plans the graph
//! with the side tables its enabled consumers need ([`CfgReq`]); the
//! first consumer that asks builds it, the others query the same graph,
//! and the next body clears it (the vectors are kept).

use std::collections::HashSet;

use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;

use super::{Cfg, CfgReq, Status};
use crate::Sema;

/// The planned graph of the body being finished.
#[derive(Default, Debug)]
pub(crate) struct BodyCfg {
    /// The function and body planned.
    at: Option<(DeclId, StmtId)>,
    /// The side tables to build (taken by the build).
    req: CfgReq,
    built: bool,
    graph: Cfg,
}

impl Sema<'_, '_> {
    /// Plans the graph of function `f`'s body `body`, dropping the
    /// previous body's (its vectors kept); nothing is built until a
    /// consumer asks ([`Sema::with_cfg`]). The deferred runtime-behavior
    /// diagnostics' anchors are `probes`; `has_fallthrough`: the body
    /// built a `[[fallthrough]]` statement.
    pub(crate) fn cfg_plan(
        &mut self,
        f: DeclId,
        body: StmtId,
        probes: HashSet<ExprId>,
        has_fallthrough: bool,
        uninit: bool,
    ) {
        let d = &self.diags;
        let in_inst = !self.inst.stack.is_empty();
        let flow = has_fallthrough
            || d.is_enabled(DiagId::sema_implicit_fallthrough)
            || (!in_inst && d.is_enabled(DiagId::sema_unreachable_fallthrough_attr));
        let scan = !in_inst
            && [
                DiagId::sema_unreachable_code,
                DiagId::sema_unreachable_code_break,
                DiagId::sema_unreachable_code_return,
                DiagId::sema_unreachable_loop_increment,
            ]
            .iter()
            .any(|&id| d.is_enabled(id));
        let c = &mut self.cfg_body;
        c.at = Some((f, body));
        c.req = CfgReq { probes, flow, scan, uninit };
        c.built = false;
        c.graph.clear();
    }

    /// Builds the planned graph unless it is built. A graph over
    /// `-fkiln-cfg-max-elems` is reported once
    /// (`-Rkiln-analysis-skipped`).
    fn cfg_ensure(&mut self) {
        if self.cfg_body.built {
            return;
        }
        let Some((f, body)) = self.cfg_body.at else { return };
        let graph = std::mem::take(&mut self.cfg_body.graph);
        let req = std::mem::take(&mut self.cfg_body.req);
        let graph = {
            // clang `CFG::buildCFG` (spec/driver.md "Time trace").
            let _t = kiln_source::trace::Scope::new("BuildCFG");
            self.build_cfg(Some(f), body, graph, req, self.cfg_max_elems)
        };
        let too_large = graph.status == Status::TooLarge;
        self.cfg_body.graph = graph;
        self.cfg_body.built = true;
        if too_large {
            let loc = self.ast.decls.get(f).loc();
            let msg = self
                .diag(DiagId::sema_analysis_skipped, loc)
                .arg(self.qualified_decl_name(f))
                .arg(self.cfg_max_elems.to_string());
            self.emit(msg);
        }
    }

    /// Runs `q` on the graph of `body`, the planned body, building it at
    /// the first query; `None` when there is no graph (clang builds none,
    /// or it is over the size cap) or `body` is not the planned body.
    pub(crate) fn with_cfg<R>(
        &mut self,
        body: StmtId,
        q: impl FnOnce(&Self, &Cfg) -> R,
    ) -> Option<R> {
        let graph = self.cfg_take(body)?;
        let r = q(self, &graph);
        self.cfg_put(graph);
        Some(r)
    }

    /// Takes the graph of `body`, the planned body, out of the cache
    /// (building it at the first query) for a consumer that emits while
    /// it reads; it goes back with [`Sema::cfg_put`]. `None` as
    /// [`Sema::with_cfg`].
    pub(crate) fn cfg_take(&mut self, body: StmtId) -> Option<Cfg> {
        if self.cfg_body.at.map(|a| a.1) != Some(body) {
            debug_assert!(false, "graph queried for a body that was not planned");
            return None;
        }
        self.cfg_ensure();
        if !self.cfg_body.graph.ok() {
            return None;
        }
        Some(std::mem::take(&mut self.cfg_body.graph))
    }

    /// Returns the graph [`Sema::cfg_take`] took.
    pub(crate) fn cfg_put(&mut self, graph: Cfg) {
        self.cfg_body.graph = graph;
    }

    /// `-fkiln-cfg-stats`: builds the planned graph and appends one line
    /// to [`Sema::cfg_out`]: the function, its blocks, elements, edges,
    /// bytes ([`Cfg::bytes`]) and the build time in nanoseconds.
    pub(crate) fn stats_body_cfg(&mut self) {
        let Some((f, _)) = self.cfg_body.at else { return };
        let start = std::time::Instant::now();
        self.cfg_ensure();
        let ns = start.elapsed().as_nanos();
        let g = &self.cfg_body.graph;
        let line = format!(
            "cfg-stats: '{}' status={:?} blocks={} elems={} edges={} bytes={} build_ns={ns}\n",
            self.cfg_title(f),
            g.status,
            g.blocks.len(),
            g.elems.len(),
            g.edges.len(),
            g.bytes(),
        );
        self.cfg_out.push_str(&line);
    }

    /// `-fkiln-dump-cfg`: appends the dump of the planned graph to
    /// [`Sema::cfg_out`] (a body with no graph prints its status).
    pub(crate) fn dump_body_cfg(&mut self) {
        self.cfg_ensure();
        if !self.cfg_body.built {
            return;
        }
        let mut out = std::mem::take(&mut self.cfg_out);
        self.dump_cfg(&self.cfg_body.graph, &mut out);
        self.cfg_out = out;
    }
}
