//! Lambda expressions ([expr.prim.lambda]; clang's `ParseLambdaExpression`,
//! WP-075).

use kiln_ast::lambda::{CaptureDefault, CaptureKind};
use kiln_diag::DiagId;
use kiln_lex::Punct;
use kiln_sema::DeclaratorContext;
use kiln_sema::declspec::{Chunk, Declarator};
use kiln_sema::lambda::{LambdaCaptureSpec, LambdaIntroducer, LambdaSpecifiers};
use kiln_source::Loc;

use crate::keywords::Kw;
use crate::parser::{PExpr, Parser};

impl Parser<'_, '_> {
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
            self.unsupported("template parameter lists of lambdas", self.loc());
            self.s.act_on_lambda_error(id);
            self.skip_lambda_rest();
            return None;
        }
        let mut specs = LambdaSpecifiers::default();
        if self.at_attr() {
            self.parse_attributes(&mut specs.attrs);
        }
        let mut d = Declarator::new(DeclaratorContext::Member, lbracket);
        d.op_name = Some(kiln_ast::names::OverloadedOperator::Call);
        let mut params = Vec::new();
        let mut variadic = false;
        let mut open = lbracket;
        let mut rparen = lbracket;
        self.s.act_on_start_prototype();
        if let Some(l) = self.eat(Punct::LParen) {
            specs.explicit_params = true;
            open = l;
            (params, variadic) = self.parse_param_clause(&mut d, false);
            rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
            if self.s.act_on_lambda_check_generic(open) {
                self.s.act_on_end_prototype();
                self.s.act_on_lambda_error(id);
                self.skip_lambda_rest();
                return None;
            }
        }
        self.parse_lambda_specifiers(&mut specs, &intro);
        let exc = if matches!(self.kw(), Some(Kw::Noexcept | Kw::Throw)) {
            self.parse_exception_spec()
        } else {
            kiln_ast::types::ExcSpec::None
        };
        if self.at_attr() {
            self.parse_attributes(&mut specs.attrs);
        }
        let mut trailing = None;
        if let Some(arrow) = self.eat(Punct::Arrow) {
            let ty = self.parse_type_id().unwrap_or(kiln_ast::QualType::ERROR);
            trailing = Some((ty, arrow));
        }
        if specs.explicit_params && self.eat_kw(Kw::Requires).is_some() {
            d.trailing_requires = self.parse_requires_clause();
        }
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
        });
        if !self.at(Punct::LBrace) {
            self.error_here(DiagId::parse_expected_lambda_body);
            self.s.act_on_lambda_error(id);
            return None;
        }
        self.s.act_on_lambda_declarator(id, d, &specs);
        let saved_stmt_expr = std::mem::replace(&mut self.in_stmt_expr, false);
        let (body, rbrace) = self.with_greater_op(true, |p| p.parse_compound(false));
        self.in_stmt_expr = saved_stmt_expr;
        Some(self.s.act_on_lambda_end(id, body, rbrace))
    }

    /// Skips an abandoned (unsupported) lambda's declarator and body.
    fn skip_lambda_rest(&mut self) {
        if self.skip_until(&[Punct::LBrace], true, true) {
            self.skip_until(&[Punct::RBrace], false, true);
        }
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
            if !first {
                if self.eat(Punct::Comma).is_none() {
                    self.error_here(DiagId::parse_expected_comma_or_rsquare);
                    return None;
                }
            }
            first = false;
            let loc = self.loc();
            let mut cap = LambdaCaptureSpec {
                kind: CaptureKind::ByCopy,
                name: None,
                loc,
                ellipsis: None,
                init: None,
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
                if self.at(Punct::Equal) {
                    self.bump();
                    cap.init = if self.at(Punct::LBrace) {
                        self.unsupported("init-captures with braced initializers", self.loc());
                        None
                    } else {
                        self.parse_assignment_expr()
                    };
                } else if self.at(Punct::LParen) || self.at(Punct::LBrace) {
                    self.unsupported("init-captures with direct initializers", self.loc());
                    let close = if self.at(Punct::LParen) { Punct::RParen } else { Punct::RBrace };
                    self.bump();
                    self.skip_until(&[close], false, true);
                }
            }
            intro.captures.push(cap);
        }
        intro.rbracket = self.bump().span.lo;
        Some(intro)
    }

    /// lambda-specifier-seq ([expr.prim.lambda.general]/1): `mutable`,
    /// `constexpr`, `consteval` and `static`, each at most once.
    fn parse_lambda_specifiers(&mut self, specs: &mut LambdaSpecifiers, intro: &LambdaIntroducer) {
        loop {
            let (slot, what): (&mut Option<Loc>, &str) = match self.kw() {
                Some(Kw::Mutable) => (&mut specs.mutable, "'mutable'"),
                Some(Kw::Static) => (&mut specs.static_, "'static'"),
                Some(Kw::Constexpr) => (&mut specs.constexpr, "'constexpr'"),
                Some(Kw::Consteval) => (&mut specs.consteval, "'consteval'"),
                _ => break,
            };
            let loc = self.tok().span.lo;
            if slot.is_some() {
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
