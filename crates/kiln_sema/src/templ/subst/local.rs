//! Local classes of templated functions ([class.local], [temp.inst]/3,
//! spec/sema-templates.md §4.4): a class defined in the body of a
//! function template, or of a member function of a class template, is
//! instantiated with the enclosing function, by the function body's own
//! substituter, so its members see the enclosing function's template
//! arguments and local declarations (clang
//! `TemplateDeclInstantiator::VisitCXXRecordDecl` with a function-local
//! context, `InstantiateClass` then the member function definitions).

use kiln_ast::decl::DeclKind;
use kiln_ast::types::TagKeyword;
use kiln_ast::{DeclId, StmtId};

use super::{DeclOrPack, Substituter};
use crate::decl::TagUse;
use crate::templ::stack::FrameKind;

/// A member function defined in a local class, defined once the
/// outermost local class is complete (the parser replays these bodies
/// after the class's `}`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalMethod {
    /// The instantiated class `f` is a member of.
    class: DeclId,
    /// The instantiated member function.
    f: DeclId,
    /// Its pattern definition.
    pdef: DeclId,
    /// The pattern body.
    body: StmtId,
}

impl Substituter<'_, '_, '_> {
    /// Local class `d` of the pattern (or a member class of a local
    /// class, when `methods` collects for the outermost one): declared in
    /// the current scope and, when `d` is a definition, defined now with
    /// its members; the outermost local class then defines its member
    /// functions.
    pub(crate) fn local_class(
        &mut self,
        d: DeclId,
        methods: Option<&mut Vec<LocalMethod>>,
    ) -> DeclId {
        let n = *self.sema.ast.decls.get(d);
        let loc = n.loc();
        let name = n.name().as_ident().filter(|_| !n.name().is_empty()).map(|s| (s, loc));
        let kw = match self.sema.ast.decls.record(d).map(|r| r.tag) {
            Some(kiln_ast::decl::TagKind::Class) => TagKeyword::Class,
            Some(kiln_ast::decl::TagKind::Union) => TagKeyword::Union,
            _ => TagKeyword::Struct,
        };
        let defined = self.sema.ast.definition(d) == Some(d);
        let use_ = if defined { TagUse::Definition } else { TagUse::Declaration };
        let x = self.sema.act_on_tag(kw, name, None, use_, 0, None, loc);
        self.sema.ast.templates.set_instantiated_from(x, d);
        self.locals.insert(d, DeclOrPack::Decl(x));
        // The body is instantiated with the function, under its own
        // context frame (clang `InstantiateClass` from
        // `VisitCXXRecordDecl` for a local class: "in instantiation of
        // member class 'L' requested here", at the class name).
        if defined && self.sema.push_inst_frame(FrameKind::MemberClass, x, d, loc) {
            // A block-scope anonymous union ([class.union.anon]): its
            // object is the declaration statement's next declaration.
            let anon = self.is_anonymous_record(d);
            if let Some(v) = methods {
                self.class_body_with(x, d, Some(v), anon);
            } else {
                let mut v = Vec::new();
                self.class_body_with(x, d, Some(&mut v), anon);
                self.sema.pop_inst_frame();
                self.local_methods(x, &v);
                return x;
            }
            self.sema.pop_inst_frame();
        }
        x
    }

    /// Collects member `x` of local class `class`, instantiated from
    /// pattern member `m`, when it is a member function `m`'s class
    /// defines.
    pub(crate) fn collect_local_method(
        &mut self,
        class: DeclId,
        x: DeclId,
        m: DeclId,
        out: &mut Vec<LocalMethod>,
    ) {
        let is_fn = |k: DeclKind| {
            matches!(
                k,
                DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            )
        };
        if !is_fn(self.sema.kind_of(m)) || !is_fn(self.sema.kind_of(x)) {
            return;
        }
        let Some(pdef) = self.sema.ast.definition(m) else { return };
        let Some(body) = self.sema.ast.decls.function(pdef).and_then(|f| f.body) else {
            return;
        };
        out.push(LocalMethod { class, f: x, pdef, body });
    }

    /// Defines the member functions `methods` of local class `outer` (and
    /// of its member classes), each in its re-entered class scope, then
    /// finishes `outer`'s class contexts.
    fn local_methods(&mut self, outer: DeclId, methods: &[LocalMethod]) {
        let poi = self.sema.ast.decls.get(outer).loc();
        self.sema.act_on_start_delayed_class(outer);
        for m in methods {
            // The member classes between `outer` and the method's class,
            // outermost first.
            let mut chain = Vec::new();
            let mut c = Some(m.class);
            while let Some(r) = c.filter(|&r| r != outer) {
                chain.push(r);
                if chain.len() > 1024 {
                    break;
                }
                c = self.sema.ast.decls.get(r).ctx();
            }
            for &r in chain.iter().rev() {
                self.sema.act_on_start_delayed_class(r);
            }
            // Under its own frame, requested at the outermost local
            // class (clang `InstantiateClassMembers` from
            // `VisitCXXRecordDecl`, DR1484).
            if self.sema.ast.definition(m.f).is_none()
                && self.sema.push_inst_frame(FrameKind::MemberFunction, m.f, m.pdef, poi)
            {
                // The enclosing function's references still waiting for
                // the end of their full-expression (a tuple-like
                // binding's `get` call) are not the member's: its body is
                // a function of its own ([basic.def.odr]/10, clang's
                // pushed function scope and evaluation context).
                let odr = std::mem::take(&mut self.sema.pending_odr);
                let caps = std::mem::take(&mut self.sema.potential_caps);
                self.function_def(m.f, m.pdef, m.body);
                self.sema.pending_odr = odr;
                self.sema.potential_caps = caps;
                self.sema.pop_inst_frame();
            }
            for &r in &chain {
                self.sema.act_on_finish_delayed_class(r);
            }
        }
        self.sema.act_on_finish_delayed_class(outer);
        self.sema.act_on_finish_class_contexts(outer);
    }
}
