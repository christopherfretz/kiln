//! `#pragma clang attribute` (spec/parse.md §2.3; clang's
//! `PragmaAttributeHandler`, `Parser::HandlePragmaAttribute` and
//! `ParsePragmaAttributeSubjectMatchRuleSet`).
//!
//! [`classify`] reads the pragma's form when the token source reads it:
//! `[NS.]push`, `[NS.]pop`, `push (...)` and `(...)`, keeping the tokens
//! between the parentheses. When the parser reaches the next token,
//! [`Parser::act_on_attr_pragma`] re-parses those tokens as one GNU or
//! C++11 attribute specifier followed by `, apply_to = rules` and hands
//! each attribute with its rules to Sema
//! ([`kiln_sema::pragma_attr`]), which applies it to the declarations
//! the rules select until the group is popped.

use kiln_ast::AttrId;
use kiln_ast::attr::AttrSyntax;
use kiln_diag::{Diag, DiagId};
use kiln_lex::{Punct, TokKind, Token};
use kiln_sema::pragma_attr::{MatchRule, attr_rules_of};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use crate::keywords::Kw;
use crate::parser::{Parser, quote};
use crate::pragma::PragmaCx;
use crate::tokens::{AttrPragma, AttrPragmaAction, Event};

/// Reads `#pragma clang attribute` whose `attribute` token is at `loc`
/// and whose tokens after it are `rest` (clang's `PragmaAttributeHandler`);
/// a malformed pragma is reported and dropped.
pub(crate) fn classify(cx: &PragmaCx<'_, '_>, loc: Loc, rest: &[Token], end: Loc) -> Option<Event> {
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let is = |i: usize, p: Punct| rest.get(i).is_some_and(|t| t.kind == TokKind::Punct(p));
    let mut i = 0;
    let mut namespace = None;
    if let Some(&Token { kind: TokKind::Identifier(s), .. }) = rest.first() {
        let w = cx.interner.as_str(s);
        if w != "push" && w != "pop" {
            namespace = Some(s);
            if !is(1, Punct::Period) {
                Diag::new(DiagId::parse_pragma_attribute_expected_period, Span::empty(at(1)))
                    .arg(w)
                    .emit(cx.diags);
                return None;
            }
            i = 2;
        }
    }
    let action = match rest.get(i).map(|t| t.kind) {
        Some(TokKind::Punct(Punct::LParen)) => {
            if namespace.is_some() {
                Diag::new(
                    DiagId::parse_pragma_attribute_namespace_on_attribute,
                    Span::empty(at(i)),
                )
                .note(DiagId::note_pragma_attribute_namespace_on_attribute, Span::empty(at(i)))
                .emit(cx.diags);
                return None;
            }
            AttrPragmaAction::Attribute
        }
        Some(TokKind::Identifier(s)) => {
            let a = match cx.interner.as_str(s) {
                "push" => AttrPragmaAction::Push,
                "pop" => AttrPragmaAction::Pop,
                w => {
                    cx.report_what(DiagId::parse_pragma_attribute_invalid_argument, w, at(i));
                    return None;
                }
            };
            i += 1;
            a
        }
        _ => {
            cx.report(DiagId::parse_pragma_attribute_expected_push_pop_paren, at(i));
            return None;
        }
    };
    let mut tokens = Vec::new();
    let mut close = end;
    if (action == AttrPragmaAction::Push && i < rest.len()) || action == AttrPragmaAction::Attribute
    {
        if !is(i, Punct::LParen) {
            cx.report_what(DiagId::parse_expected, &quote(Punct::LParen), at(i));
            return None;
        }
        i += 1;
        let mut open = 1;
        while let Some(t) = rest.get(i) {
            match t.kind {
                TokKind::Punct(Punct::LParen) => open += 1,
                TokKind::Punct(Punct::RParen) => {
                    open -= 1;
                    if open == 0 {
                        break;
                    }
                }
                _ => {}
            }
            tokens.push(*t);
            i += 1;
        }
        if tokens.is_empty() {
            cx.report(DiagId::parse_pragma_attribute_expected_attribute, at(i));
            return None;
        }
        if i >= rest.len() {
            cx.report_what(DiagId::parse_expected, &quote(Punct::RParen), end);
            return None;
        }
        close = rest[i].span.lo;
        i += 1;
    }
    if i < rest.len() {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "clang attribute", at(i));
    }
    Some(Event::PragmaAttribute(AttrPragma { loc, namespace, action, tokens, end: close }))
}

/// A pragma's attributes and their subject rules.
type Body = (Vec<Entry>, Vec<(MatchRule, Loc)>);

/// One attribute of the pragma's specifier.
struct Entry {
    /// The name as written (without a scope).
    name: String,
    /// The name token (the first token of a scoped C++11 name).
    loc: Loc,
    /// Whether `#pragma clang attribute` supports it.
    supported: bool,
    /// Its node, if kiln keeps one.
    attr: Option<AttrId>,
}

impl Parser<'_, '_> {
    /// Acts on `#pragma clang attribute` `p` (clang `HandlePragmaAttribute`).
    pub(crate) fn act_on_attr_pragma(&mut self, p: AttrPragma) {
        match p.action {
            AttrPragmaAction::Pop => self.s.act_on_pragma_attribute_pop(p.loc, p.namespace),
            AttrPragmaAction::Push if p.tokens.is_empty() => {
                self.s.act_on_pragma_attribute_push(p.loc, p.namespace);
            }
            _ => {
                self.begin_replay(p.tokens, p.end);
                let parsed = self.attr_pragma_body(p.loc);
                while !self.at_eof() {
                    self.bump();
                }
                self.end_replay();
                let Some((entries, rules)) = parsed else { return };
                if p.action == AttrPragmaAction::Push {
                    self.s.act_on_pragma_attribute_push(p.loc, p.namespace);
                }
                for e in entries {
                    self.s.act_on_pragma_attribute_attribute(p.loc, &e.name, e.loc, e.attr, &rules);
                }
            }
        }
    }

    /// The attribute specifier and subject rules of a pragma at `pragma`,
    /// or `None` after a reported error.
    fn attr_pragma_body(&mut self, pragma: Loc) -> Option<Body> {
        let entries = if self.at_cxx11_attr() {
            self.attr_pragma_cxx11()
        } else if self.at_kw(Kw::Attribute) {
            self.attr_pragma_gnu()?
        } else {
            let mut d =
                self.s.diag(DiagId::parse_pragma_attribute_expected_attribute_syntax, self.loc());
            if let TokKind::Identifier(s) = self.tok().kind
                && attr_rules_of(self.s.ast.symbols().as_str(s)).is_some()
            {
                // A bare attribute name: suggest `__attribute__((...))`.
                self.bump();
                if self.eat(Punct::LParen).is_some() {
                    self.skip_until(&[Punct::RParen], false, false);
                }
                if !self.at_eof() {
                    let at = Span::empty(self.loc());
                    d = d.note(DiagId::note_pragma_attribute_use_attribute_kw, at);
                }
            }
            self.emit(d);
            return None;
        };
        if entries.is_empty() {
            return None;
        }
        if let Some(e) = entries.iter().find(|e| !e.supported) {
            let d = self.s.diag(DiagId::parse_pragma_attribute_unsupported_attribute, pragma);
            self.emit(d.arg(&e.name));
            return None;
        }
        if self.eat(Punct::Comma).is_none() {
            self.emit(self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("','"));
            return None;
        }
        if !self.at_word("apply_to") {
            let d = self.s.diag(
                DiagId::parse_pragma_attribute_invalid_subject_set_specifier,
                self.expected_loc(),
            );
            self.emit(d);
            return None;
        }
        self.bump();
        if self.eat(Punct::Equal).is_none() {
            self.emit(self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("'='"));
            return None;
        }
        let rules = self.attr_pragma_rules()?;
        if !self.at_eof() {
            self.error_here(DiagId::parse_pragma_attribute_extra_tokens_after_attribute);
            return None;
        }
        Some((entries, rules))
    }

    /// Whether the current token is identifier `w` (not a keyword).
    fn at_word(&self, w: &str) -> bool {
        let t = self.tok();
        matches!(t.kind, TokKind::Identifier(s) if self.s.ast.symbols().as_str(s) == w)
            && self.kw_of(t).is_none()
    }

    /// The spelling of the current token if it is an identifier or a
    /// keyword (clang `getIdentifier`).
    fn word_here(&self) -> Option<String> {
        match self.tok().kind {
            TokKind::Identifier(s) => Some(self.s.ast.symbols().as_str(s).to_owned()),
            _ => None,
        }
    }

    /// `__attribute__((a, b(args)))`: each attribute's name must be an
    /// identifier.
    fn attr_pragma_gnu(&mut self) -> Option<Vec<Entry>> {
        self.bump();
        for after in ["attribute", "("] {
            if self.eat(Punct::LParen).is_none() {
                let d = self.s.diag(DiagId::parse_expected, self.expected_loc());
                self.emit(d.arg(format!("'(' after '{after}'")));
                return None;
            }
        }
        let mut out = Vec::new();
        loop {
            let t = self.tok();
            let TokKind::Identifier(name) = t.kind else {
                self.error_here(DiagId::parse_pragma_attribute_expected_attribute_name);
                return None;
            };
            if self.kw_of(t).is_some() {
                self.error_here(DiagId::parse_pragma_attribute_expected_attribute_name);
                return None;
            }
            let loc = self.bump().span.lo;
            let text = self.s.ast.symbols().as_str(name).to_owned();
            let supported = attr_rules_of(&text).is_some();
            let mut attr = None;
            if supported {
                let args = self.parse_attr_args(name)?;
                attr = self.s.act_on_attribute(AttrSyntax::Gnu, None, name, &args, loc);
            } else if self.eat(Punct::LParen).is_some() {
                self.skip_until(&[Punct::RParen], false, true);
            }
            out.push(Entry { name: text, loc, supported, attr });
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        for _ in 0..2 {
            if self.eat(Punct::RParen).is_none() {
                let d = self.s.diag(DiagId::parse_expected, self.expected_loc());
                self.emit(d.arg(quote(Punct::RParen)));
                return None;
            }
        }
        Some(out)
    }

    /// `[[a, ns::b(args)]]`: the attributes' names are read from the
    /// tokens, then the specifier is parsed as usual and each node is
    /// matched to its attribute by location.
    fn attr_pragma_cxx11(&mut self) -> Vec<Entry> {
        let mut names: Vec<(Option<Symbol>, Symbol, Loc)> = Vec::new();
        // `[[` `using ns :`? then comma-separated `ns::name(...)` items.
        let mut i = self.token_index() + 2;
        let tok = |p: &Self, i: usize| p.tok_at(i);
        if self.kw_of(tok(self, i)) == Some(Kw::Using) {
            i += 3;
        }
        let mut depth = 0u32;
        let mut start = true;
        loop {
            let t = tok(self, i);
            match t.kind {
                TokKind::Punct(Punct::LParen | Punct::LSquare | Punct::LBrace) => depth += 1,
                TokKind::Punct(Punct::RParen | Punct::RSquare | Punct::RBrace) if depth > 0 => {
                    depth -= 1;
                }
                TokKind::Eof | TokKind::Punct(Punct::RSquare) => break,
                TokKind::Punct(Punct::Comma) if depth == 0 => start = true,
                TokKind::Identifier(s) if depth == 0 && start => {
                    start = false;
                    let scoped = tok(self, i + 1).kind == TokKind::Punct(Punct::ColonColon);
                    if scoped && let TokKind::Identifier(n) = tok(self, i + 2).kind {
                        names.push((Some(s), n, t.span.lo));
                        i += 2;
                    } else {
                        names.push((None, s, t.span.lo));
                    }
                }
                _ => start = false,
            }
            i += 1;
        }
        let mut attrs = Vec::new();
        self.parse_cxx11_attr_seq(&mut attrs);
        names
            .into_iter()
            .map(|(scope, name, loc)| {
                let sym = |s: Symbol| self.s.ast.symbols().as_str(s).to_owned();
                let text = sym(name);
                let scope_ok = scope.is_none_or(|s| {
                    matches!(sym(s).as_str(), "gnu" | "__gnu__" | "clang" | "_Clang")
                });
                let attr = attrs.iter().copied().find(|&a| self.s.ast.attrs.get(a).loc == loc);
                Entry {
                    supported: scope_ok && attr_rules_of(&text).is_some(),
                    name: text,
                    loc,
                    attr,
                }
            })
            .collect()
    }

    /// `rule`, `rule(sub)`, `rule(unless(sub))` or `any(r, ...)` (clang
    /// `ParsePragmaAttributeSubjectMatchRuleSet`), each rule once, in
    /// rule order.
    fn attr_pragma_rules(&mut self) -> Option<Vec<(MatchRule, Loc)>> {
        let mut rules: Vec<(MatchRule, Loc)> = Vec::new();
        let any = self.at_word("any");
        let mut any_open = Loc::INVALID;
        if any {
            self.bump();
            any_open = self.loc();
            if self.eat(Punct::LParen).is_none() {
                self.emit(self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("'('"));
                return None;
            }
        }
        loop {
            let Some(name) = self.word_here() else {
                self.error_here(DiagId::parse_pragma_attribute_expected_subject_identifier);
                return None;
            };
            let Some(primary) = MatchRule::primary(&name) else {
                let d =
                    self.s.diag(DiagId::parse_pragma_attribute_unknown_subject_rule, self.loc());
                self.emit(d.arg(&name));
                return None;
            };
            let rule_loc = self.bump().span.lo;
            let open = self.loc();
            let rule = if self.at(Punct::LParen) {
                self.bump();
                let sub = self.attr_pragma_sub_rule(primary, &name)?;
                self.expect_closing(Punct::RParen, Punct::LParen, open);
                sub
            } else if primary.is_abstract() {
                self.emit(self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("'('"));
                return None;
            } else {
                primary
            };
            if rules.iter().any(|&(r, _)| r == rule) {
                let d = self.s.diag(DiagId::parse_pragma_attribute_duplicate_subject, rule_loc);
                self.emit(d.arg(rule.spelling()));
            } else {
                rules.push((rule, rule_loc));
            }
            if !(any && self.eat(Punct::Comma).is_some()) {
                break;
            }
        }
        if any && self.eat(Punct::RParen).is_none() {
            self.expect_closing(Punct::RParen, Punct::LParen, any_open);
            return None;
        }
        rules.sort_by_key(|&(r, _)| r);
        Some(rules)
    }

    /// The sub-rule after `primary(` (named `name`).
    fn attr_pragma_sub_rule(&mut self, primary: MatchRule, name: &str) -> Option<MatchRule> {
        let supported = |r: MatchRule| match r.sub_rules() {
            Some(s) => format!("supports the following sub-rules: {s}"),
            None => "does not support sub-rules".to_owned(),
        };
        let expected = |p: &mut Self, at: Loc| {
            let d =
                p.s.diag(DiagId::parse_pragma_attribute_expected_subject_sub_identifier, at)
                    .arg(name)
                    .arg(supported(primary));
            p.emit(d);
        };
        let unknown = |p: &mut Self, sub: &str, at: Loc| {
            let d =
                p.s.diag(DiagId::parse_pragma_attribute_unknown_subject_sub_rule, at)
                    .arg(u8::from(primary.sub_rules().is_some()))
                    .arg(sub)
                    .arg(name)
                    .arg(supported(primary));
            p.emit(d);
        };
        let Some(sub) = self.word_here() else {
            expected(self, self.loc());
            return None;
        };
        if sub == "unless" {
            let unless_loc = self.bump().span.lo;
            let open = self.loc();
            if self.eat(Punct::LParen).is_none() {
                self.emit(self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("'('"));
                return None;
            }
            let Some(inner) = self.word_here() else {
                expected(self, unless_loc);
                return None;
            };
            let Some(r) = primary.sub_rule(&inner, true) else {
                unknown(self, &format!("unless({inner})"), unless_loc);
                return None;
            };
            self.bump();
            self.expect_closing(Punct::RParen, Punct::LParen, open);
            return Some(r);
        }
        let Some(r) = primary.sub_rule(&sub, false) else {
            unknown(self, &sub, self.loc());
            return None;
        };
        self.bump();
        Some(r)
    }
}
