//! consteval-block-declaration ([dcl.pre]/1, P3289R1; spec/consteval-blocks.md
//! §1): `consteval` compound-statement, a block-declaration and a
//! member-declaration. The block is modelled as the call of a synthesized
//! `[] -> void consteval { ... }` lambda, built in a manifestly
//! constant-evaluated context and evaluated by sema
//! ([`kiln_sema::Sema::act_on_consteval_block`]).

use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_lex::{Punct, Std};
use kiln_sema::DeclaratorContext;
use kiln_sema::declspec::{Chunk, Declarator};
use kiln_sema::lambda::{LambdaIntroducer, LambdaSpecifiers};
use kiln_source::Loc;

use crate::keywords::Kw;
use crate::parser::Parser;

impl Parser<'_, '_> {
    /// Whether the parser is at a consteval-block-declaration: `consteval`
    /// followed by `{` (anything else after `consteval` is the
    /// decl-specifier).
    pub(crate) fn at_consteval_block(&self) -> bool {
        self.at_kw(Kw::Consteval) && self.peek_is(1, Punct::LBrace)
    }

    /// consteval-block-declaration, at `consteval {`. C++26 only (no
    /// `-freflection` needed).
    pub(crate) fn parse_consteval_block(&mut self) -> DeclId {
        let loc = self.bump().span.lo;
        if self.std < Std::Cxx26 {
            let d = self.s.diag(DiagId::parse_consteval_block_cxx26, loc);
            self.emit(d);
        }
        self.s.enter_constant_evaluated();
        let call = self.parse_consteval_block_call(loc);
        self.s.exit_constant_evaluated(Some(call));
        self.s.act_on_consteval_block(call, loc)
    }

    /// A consteval block as a statement at `loc` (a block-declaration's
    /// declaration-statement); attributes at `lead` are prohibited.
    pub(crate) fn parse_consteval_block_stmt(&mut self, lead: Option<Loc>, loc: Loc) -> StmtId {
        self.prohibit_attrs_at(lead);
        let d = self.parse_consteval_block();
        self.s.act_on_decl_stmt(&[d], loc)
    }

    /// The block's compound statement as the body of a `consteval` lambda
    /// with no captures and no parameters returning `void`, and the call
    /// of that lambda.
    fn parse_consteval_block_call(&mut self, loc: Loc) -> ExprId {
        let intro = LambdaIntroducer { lbracket: loc, rbracket: loc, ..Default::default() };
        let id = self.s.act_on_lambda_start(&intro);
        let specs = LambdaSpecifiers { consteval: Some(loc), ..Default::default() };
        let mut d = Declarator::new(DeclaratorContext::Member, loc);
        d.op_name = Some(kiln_ast::names::OverloadedOperator::Call);
        self.s.act_on_start_prototype();
        self.s.act_on_end_prototype();
        let params = self.s.act_on_param_list(Vec::new());
        d.chunks.push(Chunk::Function {
            params,
            variadic: false,
            loc,
            rparen: loc,
            exc: kiln_ast::types::ExcSpec::None,
            quals: kiln_ast::types::Quals::NONE,
            ref_qual: kiln_ast::types::RefQual::None,
            trailing: Some((QualType::VOID, loc)),
            ambiguous: false,
            no_proto: false,
        });
        self.s.act_on_lambda_declarator(id, d, &specs);
        let saved_stmt_expr = std::mem::replace(&mut self.in_stmt_expr, false);
        let typos = self.s.suspend_delayed_typos();
        let (body, rbrace) = self.with_greater_op(true, |p| p.parse_compound(false));
        self.s.resume_delayed_typos(typos);
        self.in_stmt_expr = saved_stmt_expr;
        self.sync_attr_macros();
        let lambda = self.s.act_on_lambda_end(id, body, rbrace);
        self.s.act_on_call(lambda, &[], rbrace, rbrace)
    }
}
