//! Lambda expressions ([expr.prim.lambda]; clang's `ParseLambdaExpression`,
//! WP-075).

use kiln_ast::decl::InitStyle;
use kiln_ast::lambda::{CaptureDefault, CaptureKind};
use kiln_diag::DiagId;
use kiln_lex::{Punct, Std};
use kiln_sema::DeclSite;
use kiln_sema::DeclaratorContext;
use kiln_sema::Initializer;
use kiln_sema::declspec::{Chunk, Declarator};
use kiln_sema::lambda::{LambdaCaptureSpec, LambdaIntroducer, LambdaSpecifiers};
use kiln_source::Loc;

use crate::keywords::Kw;
use crate::parser::{PExpr, Parser};

impl Parser<'_, '_> {
    /// Before C++23 an attribute-specifier-seq after the
    /// lambda-introducer is an extension (P2173R1; clang
    /// `ext_decl_attrs_on_lambda`).
    fn warn_cxx23_lambda_attrs(&mut self) {
        if self.std < Std::Cxx23 && (self.at_cxx11_attr() || self.at_kw(Kw::Alignas)) {
            let w = self.s.diag(DiagId::parse_ext_decl_attrs_on_lambda, self.loc());
            self.emit(w);
        }
    }

    /// [expr.prim.lambda.general]/1: before C++23 (P1102R2) a
    /// lambda-specifier, trailing return type or attribute needs the
    /// parameter clause (clang `ext_lambda_missing_parens`).
    fn warn_cxx23_lambda_missing_parens(&mut self) {
        if self.std < Std::Cxx23 && self.at_lambda_specifier_without_params() {
            let at = self.loc();
            let w = self
                .s
                .diag(DiagId::parse_ext_lambda_missing_parens, at)
                .fixit(kiln_diag::FixIt::insert(at, "() "));
            self.emit(w);
        }
    }

    /// [expr.prim.lambda.general]/1: `<` template-parameter-list `>`
    /// requires-clause?, at the `<`.
    fn parse_lambda_template_head(&mut self) {
        let l_angle = self.bump().span.lo;
        self.s.act_on_lambda_template_params_start(l_angle);
        let r_angle = self.parse_template_param_list_tail();
        let requires = if self.at_kw(Kw::Requires) {
            self.bump();
            self.parse_requires_clause()
        } else {
            None
        };
        let _ = self.s.act_template_head(requires, r_angle);
    }

    /// lambda-expression ([expr.prim.lambda.general]/1). At the `[`.
    pub(crate) fn parse_lambda_expr(&mut self) -> PExpr {
        let Some(intro) = self.parse_lambda_introducer() else {
            // clang: each skip consumes its stop token, so the last one
            // stops at the body's first `;`.
            self.skip_until(&[Punct::RSquare], true, true);
            self.skip_until(&[Punct::LBrace], true, true);
            self.skip_until(&[Punct::RBrace], true, true);
            return None;
        };
        let id = self.s.act_on_lambda_start(&intro);
        let lbracket = intro.lbracket;
        if self.at(Punct::Less) {
            self.parse_lambda_template_head();
        }
        let mut specs = LambdaSpecifiers::default();
        // [expr.prim.lambda.general]/1: only an attribute-specifier-seq
        // precedes the parameter list (clang `ParseLambdaExpression
        // AfterIntroducer`); a GNU attribute here belongs to the
        // lambda-specifier position below, so `[] __attribute__((x)) ()`
        // is `expected body of lambda expression` at the `(`. Before C++23
        // the seq is an extension (P2173R1; clang `ext_decl_attrs_on_lambda`).
        self.warn_cxx23_lambda_attrs();
        self.parse_cxx11_attr_seq(&mut specs.attrs);
        let mut d = Declarator::new(DeclaratorContext::Member, lbracket);
        d.op_name = Some(kiln_ast::names::OverloadedOperator::Call);
        d.lambda = true;
        let mut params = Vec::new();
        let mut variadic = false;
        let mut open = lbracket;
        let mut rparen = lbracket;
        self.s.act_on_start_prototype();
        if let Some(l) = self.eat(Punct::LParen) {
            specs.explicit_params = true;
            open = l;
            (params, variadic) = self.parse_param_clause(&mut d, false, DeclSite::LambdaParam);
            rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        } else {
            self.warn_cxx23_lambda_missing_parens();
        }
        // clang `ParseLambdaExpressionAfterIntroducer`: GNU and `__declspec`
        // attributes may precede the lambda-specifier-seq (GCC and MSVC
        // compatibility); they appertain to the call operator.
        while matches!(self.kw(), Some(Kw::Attribute | Kw::Declspec)) {
            let before = self.pos();
            self.parse_attributes(&mut specs.attrs);
            if self.pos() == before {
                break;
            }
        }
        self.parse_lambda_specifiers(&mut specs, &intro);
        let exc = if matches!(self.kw(), Some(Kw::Noexcept | Kw::Throw)) {
            self.parse_exception_spec()
        } else {
            kiln_ast::types::ExcSpec::None
        };
        // [expr.prim.lambda.general]/1, [dcl.fct]/1: the
        // attribute-specifier-seq after the noexcept-specifier appertains
        // to the call operator's type, so a declaration attribute there
        // is rejected as in `void f() noexcept [[noreturn]];` (clang
        // `ParseLambdaSpecifiers`). A GNU attribute is not accepted after
        // the lambda-specifiers or the noexcept-specifier: it ends the
        // declarator, `expected body of lambda expression` (WP-752).
        let mut type_attrs = Vec::new();
        self.parse_cxx11_attr_seq(&mut type_attrs);
        let lb = self.s.take_fn_type_lifetimebound(&mut type_attrs);
        self.s.act_on_type_position_attrs(&mut type_attrs, true);
        specs.attrs.extend(type_attrs);
        specs.attrs.extend(lb);
        let mut trailing = None;
        if let Some(arrow) = self.eat(Punct::Arrow) {
            let tloc = self.loc();
            let mut ty = self.parse_type_id_no_ctad().unwrap_or(kiln_ast::QualType::ERROR);
            if self.s.gnu_auto_type_return_check(ty, tloc) {
                ty = kiln_ast::QualType::ERROR;
            }
            trailing = Some((ty, arrow));
        }
        if specs.explicit_params && self.eat_kw(Kw::Requires).is_some() {
            d.trailing_requires = self.parse_requires_clause();
        }
        // [expr.prim.lambda.general]/1: the seq ends the lambda-declarator;
        // replayed once the call operator is declared.
        let contracts = self.capture_contract_specs();
        self.s.act_on_end_prototype();
        let params = self.s.act_on_param_list(params);
        d.chunks.push(Chunk::Function {
            params,
            variadic,
            loc: open,
            rparen,
            exc,
            quals: kiln_ast::types::Quals::NONE,
            ref_qual: kiln_ast::types::RefQual::None,
            trailing,
            ambiguous: false,
            no_proto: false,
        });
        if !self.at(Punct::LBrace) {
            self.error_here(DiagId::parse_expected_lambda_body);
            self.s.act_on_lambda_error(id);
            return None;
        }
        self.s.act_on_lambda_declarator(id, d, &specs);
        // [dcl.contract.res]/2: a deduced return type is known after the
        // body.
        let op = self.s.current_lambda_call_op();
        let late = match (op, contracts) {
            (Some(op), Some(t)) if self.s.contract_return_undeduced(op) => Some((op, t)),
            (Some(op), Some(t)) => {
                self.replay_lambda_contract_specs(op, t);
                None
            }
            _ => None,
        };
        let saved_stmt_expr = std::mem::replace(&mut self.in_stmt_expr, false);
        // The body's statements are full-expressions of their own: their
        // delayed typos are not held for the enclosing statement (WP-330).
        let typos = self.s.suspend_delayed_typos();
        let (body, rbrace) = self.with_greater_op(true, |p| p.parse_compound(false));
        if let Some((op, t)) = late {
            self.replay_lambda_contract_specs(op, t);
        }
        self.s.resume_delayed_typos(typos);
        self.in_stmt_expr = saved_stmt_expr;
        self.sync_attr_macros();
        Some(self.s.act_on_lambda_end(id, body, rbrace))
    }

    /// lambda-introducer ([expr.prim.lambda.general]/1,
    /// [expr.prim.lambda.capture]/1); `None` after an error (diagnosed).
    fn parse_lambda_introducer(&mut self) -> Option<LambdaIntroducer> {
        let lbracket = self.bump().span.lo;
        let mut intro = LambdaIntroducer { lbracket, ..LambdaIntroducer::default() };
        let mut first = true;
        let next_ends = |p: &Self| p.peek_is(1, Punct::Comma) || p.peek_is(1, Punct::RSquare);
        if self.at(Punct::Amp) && next_ends(self) {
            intro.default = CaptureDefault::ByRef;
            intro.default_loc = self.bump().span.lo;
            first = false;
        } else if self.at(Punct::Equal) {
            intro.default = CaptureDefault::ByCopy;
            intro.default_loc = self.bump().span.lo;
            first = false;
        }
        while !self.at(Punct::RSquare) {
            if !first && self.eat(Punct::Comma).is_none() {
                self.error_here(DiagId::parse_expected_comma_or_rsquare);
                return None;
            }
            first = false;
            let loc = self.loc();
            let mut cap = LambdaCaptureSpec {
                kind: CaptureKind::ByCopy,
                name: None,
                loc,
                end: loc,
                ellipsis: None,
                init: None,
                var: None,
            };
            if self.at(Punct::Star) {
                self.bump();
                if self.eat_kw(Kw::This).is_none() {
                    self.error_here(DiagId::parse_expected_star_this_capture);
                    return None;
                }
                cap.kind = CaptureKind::StarThis;
            } else if self.eat_kw(Kw::This).is_some() {
                cap.kind = CaptureKind::This;
            } else if (self.at(Punct::Amp) || self.at(Punct::Equal))
                && next_ends(self)
                && intro.default == CaptureDefault::None
            {
                self.error_here(DiagId::parse_capture_default_first);
                return None;
            } else {
                let mut ellipsis = self.eat(Punct::Ellipsis);
                if self.eat(Punct::Amp).is_some() {
                    cap.kind = CaptureKind::ByRef;
                    ellipsis = ellipsis.or(self.eat(Punct::Ellipsis));
                }
                ellipsis = ellipsis.or(self.eat(Punct::Ellipsis));
                if let Some(sym) = self.ident() {
                    let l = self.bump().span.lo;
                    cap.name = Some((sym, l));
                } else if self.at_kw(Kw::This) {
                    self.error_here(DiagId::parse_this_captured_by_reference);
                    return None;
                } else {
                    self.error_here(DiagId::parse_expected_capture);
                    return None;
                }
                ellipsis = ellipsis.or(self.eat(Punct::Ellipsis));
                cap.ellipsis = ellipsis;
                cap.init = self.parse_init_capture_initializer();
            }
            cap.end = self.prev_lo();
            intro.captures.push(cap);
        }
        intro.rbracket = self.bump().span.lo;
        Some(intro)
    }

    /// The *initializer* of an init-capture, if one follows the
    /// identifier ([expr.prim.lambda.capture]/1, /6): `= initializer-clause`,
    /// `= braced-init-list`, `( expression-list )` or `braced-init-list`,
    /// with its syntax as for a variable's initializer (clang
    /// `ParseLambdaIntroducer`). A malformed one (diagnosed) is an error
    /// expression, which Sema skips, as clang keeps the capture with an
    /// invalid initializer.
    fn parse_init_capture_initializer(&mut self) -> Option<(InitStyle, Initializer)> {
        let at = self.loc();
        let (style, init) = if self.eat(Punct::Equal).is_some() {
            let init = if self.at(Punct::LBrace) {
                self.parse_init_list().ok().map(Initializer::List)
            } else {
                self.parse_assignment_expr().map(Initializer::Expr)
            };
            (InitStyle::C, init)
        } else if self.at(Punct::LParen) {
            // clang parses an expression-list, which cannot be empty.
            let init = if self.peek_is(1, Punct::RParen) {
                self.bump();
                self.error_here(DiagId::parse_expected_expression);
                self.bump();
                None
            } else {
                let i = self.parse_initializer(Some(InitStyle::Call), false).ok().flatten();
                if i.is_none() {
                    // clang `Parens.skipToEnd()`.
                    self.skip_until(&[Punct::RParen], false, true);
                }
                i
            };
            (InitStyle::Call, init)
        } else if self.at(Punct::LBrace) {
            (InitStyle::List, self.parse_init_list().ok().map(Initializer::List))
        } else {
            return None;
        };
        let init = init.unwrap_or_else(|| Initializer::Expr(self.s.act_on_error_expr(&[], at)));
        Some((style, init))
    }

    /// After a lambda's introducer, template head and attributes with no
    /// `(`: a token clang's `ParseLambdaExpressionAfterIntroducer` takes
    /// as the start of the declarator without a parameter clause.
    fn at_lambda_specifier_without_params(&self) -> bool {
        self.at(Punct::Arrow)
            || self.at_cxx11_attr()
            || matches!(
                self.kw(),
                Some(
                    Kw::Mutable
                        | Kw::Attribute
                        | Kw::Constexpr
                        | Kw::Consteval
                        | Kw::Static
                        | Kw::Requires
                        | Kw::Noexcept
                )
            )
    }

    /// lambda-specifier-seq ([expr.prim.lambda.general]/1): `mutable`,
    /// `constexpr`, `consteval` and `static`, each at most once.
    fn parse_lambda_specifiers(&mut self, specs: &mut LambdaSpecifiers, intro: &LambdaIntroducer) {
        loop {
            let kw = self.kw();
            // clang `DeclSpec::SetConstexprSpec`: `constexpr` and
            // `consteval` do not combine.
            let clash = match kw {
                Some(Kw::Constexpr) => specs.consteval.map(|_| "consteval"),
                Some(Kw::Consteval) => specs.constexpr.map(|_| "constexpr"),
                _ => None,
            };
            let (slot, what): (&mut Option<Loc>, &str) = match kw {
                Some(Kw::Mutable) => (&mut specs.mutable, "'mutable'"),
                Some(Kw::Static) => (&mut specs.static_, "'static'"),
                Some(Kw::Constexpr) => (&mut specs.constexpr, "'constexpr'"),
                Some(Kw::Consteval) => (&mut specs.consteval, "'consteval'"),
                _ => break,
            };
            let loc = self.tok().span.lo;
            if let Some(prev) = clash {
                let d = self.s.diag(DiagId::parse_invalid_decl_spec_combination, loc).arg(prev);
                self.emit(d);
            } else if slot.is_some() {
                let d = self.s.diag(DiagId::parse_lambda_decl_specifier_repeated, loc).arg(what);
                self.emit(d);
            } else {
                *slot = Some(loc);
            }
            self.bump();
        }
        // [expr.prim.lambda.general]/4.
        if let Some(s) = specs.static_ {
            if specs.mutable.is_some() {
                self.emit(self.s.diag(DiagId::parse_static_mutable_lambda, s));
            }
            if intro.default != CaptureDefault::None || !intro.captures.is_empty() {
                self.emit(self.s.diag(DiagId::parse_static_lambda_captures, s));
            }
        }
    }
}
