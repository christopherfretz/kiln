//! `.x` files: the Tcl a `c-torture-execute` test sources before it runs
//! (spec/harness.md "GCC testsuites"). The interpreter knows the commands
//! those files use, `set`, `lappend`, `if`/`elseif`/`else`, `return`,
//! `continue`, `global` and `load_lib`, and the expression forms of their
//! conditions, with `istarget`, `check_effective_target_*` and `string
//! match` as command substitutions. `&&` and `||` short-circuit, as in Tcl,
//! so a branch for another target may call what the interpreter lacks.
//!
//! A `.x` file that returns 1 skips its test; `additional_flags` is the
//! suite's base flags as the file leaves them; `torture_execute_xfail`
//! marks an expected failure on matching targets; and
//! `torture_eval_before_compile` is re-run per option set with `option`
//! bound, where `continue` skips that option set.

use std::collections::BTreeMap;

use super::target::Target;
use super::tcl::{self, Word};

/// What sourcing a `.x` file left behind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct XResult {
    /// The file returned 1: the test does not run.
    pub skip: bool,
    pub additional_flags: String,
    /// `torture_execute_xfail` was set to a pattern matching the target.
    pub xfail: bool,
    pub before_compile: Option<String>,
}

/// Sources `text` with `additional_flags` bound to `base_flags`.
pub fn eval(text: &str, target: &Target, base_flags: &str) -> Result<XResult, String> {
    let mut i = Interp::new(target);
    i.vars.insert("additional_flags".into(), base_flags.to_owned());
    let flow = i.script(text)?;
    let ret = match flow {
        Flow::Return(v) => v,
        Flow::Normal => "0".to_owned(),
        Flow::Continue => return Err("`continue` outside a loop".into()),
    };
    let xfail = i.vars.get("torture_execute_xfail").is_some_and(|p| target.is(p));
    Ok(XResult {
        skip: truth(&ret)?,
        additional_flags: i.vars.get("additional_flags").cloned().unwrap_or_default(),
        xfail,
        before_compile: i.vars.get("torture_eval_before_compile").cloned(),
    })
}

/// Whether option set `option` survives `torture_eval_before_compile`.
pub fn option_allowed(body: &str, target: &Target, option: &str) -> Result<bool, String> {
    let mut i = Interp::new(target);
    i.vars.insert("option".into(), option.to_owned());
    Ok(!matches!(i.script(body)?, Flow::Continue))
}

enum Flow {
    Normal,
    Return(String),
    Continue,
}

struct Interp<'t> {
    target: &'t Target,
    vars: BTreeMap<String, String>,
}

fn truth(v: &str) -> Result<bool, String> {
    let v = v.trim();
    if let Ok(n) = v.parse::<i64>() {
        return Ok(n != 0);
    }
    match v {
        "true" | "yes" | "on" => Ok(true),
        "false" | "no" | "off" | "" => Ok(false),
        _ => Err(format!("expected a boolean, got `{v}`")),
    }
}

impl<'t> Interp<'t> {
    fn new(target: &'t Target) -> Self {
        let mut vars = BTreeMap::new();
        vars.insert("target_triplet".into(), target.triplet.clone());
        vars.insert("CFLAGS".into(), String::new());
        Self { target, vars }
    }

    fn script(&mut self, s: &str) -> Result<Flow, String> {
        for cmd in tcl::commands(s) {
            match self.command(&cmd)? {
                Flow::Normal => {}
                f => return Ok(f),
            }
        }
        Ok(Flow::Normal)
    }

    fn command(&mut self, w: &[Word]) -> Result<Flow, String> {
        let name = self.subst(&w[0])?;
        match name.as_str() {
            "load_lib" | "global" => Ok(Flow::Normal),
            "set" => {
                let var = self.subst(w.get(1).ok_or("`set` needs a name")?)?;
                if let Some(v) = w.get(2) {
                    let v = self.subst(v)?;
                    self.vars.insert(var, v);
                }
                Ok(Flow::Normal)
            }
            "lappend" => {
                let var = self.subst(w.get(1).ok_or("`lappend` needs a name")?)?;
                let mut cur = self.vars.get(&var).cloned().unwrap_or_default();
                for x in &w[2..] {
                    let x = self.subst(x)?;
                    if !cur.is_empty() {
                        cur.push(' ');
                    }
                    cur.push_str(&x);
                }
                self.vars.insert(var, cur);
                Ok(Flow::Normal)
            }
            "return" => Ok(Flow::Return(match w.get(1) {
                Some(v) => self.subst(v)?,
                None => String::new(),
            })),
            "continue" => Ok(Flow::Continue),
            "if" => self.if_command(&w[1..]),
            n => Err(format!("unsupported command `{n}`")),
        }
    }

    fn if_command(&mut self, w: &[Word]) -> Result<Flow, String> {
        let mut i = 0;
        loop {
            let cond = w.get(i).ok_or("`if` needs a condition")?;
            i += 1;
            if w.get(i).is_some_and(|x| x.text() == "then") {
                i += 1;
            }
            let body = w.get(i).ok_or("`if` needs a body")?;
            i += 1;
            if self.condition(cond.text())? {
                return self.script(body.text());
            }
            match w.get(i).map(Word::text) {
                None => return Ok(Flow::Normal),
                Some("elseif") => i += 1,
                Some("else") => {
                    let body = w.get(i + 1).ok_or("`else` needs a body")?;
                    return self.script(body.text());
                }
                Some(x) => return Err(format!("unexpected `{x}` in `if`")),
            }
        }
    }

    /// `$name`, `${name}`, `[command]` and backslash substitution of a
    /// quoted or bare word; a braced word is literal.
    fn subst(&mut self, w: &Word) -> Result<String, String> {
        match w {
            Word::Braced(s) => Ok(s.clone()),
            Word::Quoted(s) | Word::Bare(s) => self.subst_str(s),
        }
    }

    fn subst_str(&mut self, s: &str) -> Result<String, String> {
        let c: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < c.len() {
            match c[i] {
                '\\' if i + 1 < c.len() => {
                    out.push(match c[i + 1] {
                        'n' => '\n',
                        't' => '\t',
                        x => x,
                    });
                    i += 2;
                }
                '$' => {
                    let (name, next) = var_name(&c, i + 1);
                    if name.is_empty() {
                        out.push('$');
                        i += 1;
                        continue;
                    }
                    let v = self.vars.get(&name).ok_or_else(|| format!("no variable `{name}`"))?;
                    out.push_str(v);
                    i = next;
                }
                '[' => {
                    let end = bracket_end(&c, i).ok_or("unbalanced `[`")?;
                    let inner: String = c[i + 1..end].iter().collect();
                    out.push_str(&self.call_text(&inner)?);
                    i = end + 1;
                }
                x => {
                    out.push(x);
                    i += 1;
                }
            }
        }
        Ok(out)
    }

    /// The value of a command substitution.
    fn call_text(&mut self, inner: &str) -> Result<String, String> {
        let cmds = tcl::commands(inner);
        let [cmd] = cmds.as_slice() else {
            return Err(format!("unsupported command substitution `[{inner}]`"));
        };
        let mut args = Vec::new();
        for w in cmd {
            args.push(self.subst(w)?);
        }
        let b = |v: bool| if v { "1".to_owned() } else { "0".to_owned() };
        match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
            ["istarget", p] => Ok(b(self.target.is(p))),
            ["string", "match", p, s] => Ok(b(tcl::glob(p, s))),
            [name] if name.starts_with("check_effective_target_") => {
                Ok(b(self.target.effective(&name["check_effective_target_".len()..])?))
            }
            _ => Err(format!("unsupported command `{}`", args.first().map_or("", String::as_str))),
        }
    }

    /// A Tcl `expr` condition, short-circuiting `&&` and `||`.
    fn condition(&mut self, s: &str) -> Result<bool, String> {
        let toks = tokens(s)?;
        let mut p = Parser { t: &toks, i: 0 };
        let e = p.or()?;
        if p.i != toks.len() {
            return Err(format!("bad condition `{}`", s.trim()));
        }
        truth(&self.value(&e)?)
    }

    fn value(&mut self, e: &Expr) -> Result<String, String> {
        let b = |v: bool| if v { "1".to_owned() } else { "0".to_owned() };
        Ok(match e {
            Expr::Atom(t) => match t {
                Tok::Cmd(inner) => self.call_text(inner)?,
                Tok::Str(s) => self.subst_str(s)?,
                Tok::Lit(s) => s.clone(),
                _ => return Err("operator where a value belongs".into()),
            },
            Expr::Not(x) => b(!truth(&self.value(x)?)?),
            Expr::And(x, y) => b(truth(&self.value(x)?)? && truth(&self.value(y)?)?),
            Expr::Or(x, y) => b(truth(&self.value(x)?)? || truth(&self.value(y)?)?),
            Expr::Eq(x, y, eq) => b((self.value(x)? == self.value(y)?) == *eq),
        })
    }
}

/// A variable name after `$`: `name` (word characters and `::`) or `{name}`.
fn var_name(c: &[char], from: usize) -> (String, usize) {
    if c.get(from) == Some(&'{') {
        let end = c[from..].iter().position(|&x| x == '}').map_or(c.len(), |p| from + p);
        return (c[from + 1..end].iter().collect(), end + 1);
    }
    let mut j = from;
    while j < c.len() && (c[j].is_alphanumeric() || c[j] == '_' || c[j] == ':') {
        j += 1;
    }
    (c[from..j].iter().collect(), j)
}

/// The index of the `]` closing the `[` at `open`.
fn bracket_end(c: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut quoted = false;
    let mut braces = 0usize;
    let mut i = open;
    while i < c.len() {
        match c[i] {
            '\\' => i += 1,
            '"' if braces == 0 => quoted = !quoted,
            '{' if !quoted => braces += 1,
            '}' if !quoted => braces = braces.saturating_sub(1),
            '[' if !quoted && braces == 0 => depth += 1,
            ']' if !quoted && braces == 0 => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Not,
    And,
    Or,
    Open,
    Close,
    Eq(bool),
    /// `[...]`, its inside.
    Cmd(String),
    /// `"..."` or `$var` (substituted when evaluated).
    Str(String),
    /// A number, a bare word or `{...}`.
    Lit(String),
}

fn tokens(s: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        match c[i] {
            x if x.is_whitespace() => i += 1,
            _ if two == "&&" => {
                out.push(Tok::And);
                i += 2;
            }
            _ if two == "||" => {
                out.push(Tok::Or);
                i += 2;
            }
            _ if two == "==" || two == "!=" => {
                out.push(Tok::Eq(two == "=="));
                i += 2;
            }
            '!' => {
                out.push(Tok::Not);
                i += 1;
            }
            '(' => {
                out.push(Tok::Open);
                i += 1;
            }
            ')' => {
                out.push(Tok::Close);
                i += 1;
            }
            '[' => {
                let end = bracket_end(&c, i).ok_or("unbalanced `[`")?;
                out.push(Tok::Cmd(c[i + 1..end].iter().collect()));
                i = end + 1;
            }
            '"' => {
                let mut j = i + 1;
                while j < c.len() && c[j] != '"' {
                    if c[j] == '\\' {
                        j += 1;
                    }
                    j += 1;
                }
                out.push(Tok::Str(c[i + 1..j.min(c.len())].iter().collect()));
                i = j + 1;
            }
            '{' => {
                let w = tcl::list(&c[i..].iter().collect::<String>());
                let first = w.first().ok_or("unbalanced `{`")?.text().to_owned();
                out.push(Tok::Lit(first.clone()));
                i += first.chars().count() + 2;
            }
            '$' => {
                let (name, next) = var_name(&c, i + 1);
                out.push(Tok::Str(c[i..next.min(c.len())].iter().collect()));
                if name.is_empty() {
                    return Err("bare `$`".into());
                }
                i = next;
            }
            _ => {
                let mut j = i;
                while j < c.len() && (c[j].is_alphanumeric() || c[j] == '_' || c[j] == '.') {
                    j += 1;
                }
                if j == i {
                    return Err(format!("unexpected `{}` in a condition", c[i]));
                }
                out.push(Tok::Lit(c[i..j].iter().collect()));
                i = j;
            }
        }
    }
    Ok(out)
}

enum Expr {
    Atom(Tok),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>, bool),
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
}

impl Parser<'_> {
    fn or(&mut self) -> Result<Expr, String> {
        let mut e = self.and()?;
        while self.t.get(self.i) == Some(&Tok::Or) {
            self.i += 1;
            e = Expr::Or(Box::new(e), Box::new(self.and()?));
        }
        Ok(e)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut e = self.eq()?;
        while self.t.get(self.i) == Some(&Tok::And) {
            self.i += 1;
            e = Expr::And(Box::new(e), Box::new(self.eq()?));
        }
        Ok(e)
    }

    fn eq(&mut self) -> Result<Expr, String> {
        let e = self.unary()?;
        if let Some(Tok::Eq(eq)) = self.t.get(self.i) {
            let eq = *eq;
            self.i += 1;
            return Ok(Expr::Eq(Box::new(e), Box::new(self.unary()?), eq));
        }
        Ok(e)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        let t = self.t.get(self.i).ok_or("condition ends early")?.clone();
        self.i += 1;
        match t {
            Tok::Not => Ok(Expr::Not(Box::new(self.unary()?))),
            Tok::Open => {
                let e = self.or()?;
                if self.t.get(self.i) != Some(&Tok::Close) {
                    return Err("missing `)`".into());
                }
                self.i += 1;
                Ok(e)
            }
            Tok::Cmd(_) | Tok::Str(_) | Tok::Lit(_) => Ok(Expr::Atom(t)),
            _ => Err("operator where a value belongs".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm64() -> Target {
        Target::for_arch("aarch64", false)
    }

    #[test]
    fn return_values_skip() {
        let t = arm64();
        let r = eval("if [istarget \"avr-*-*\"] {\n    return 1\n}\nreturn 0\n", &t, "-fno-inline")
            .unwrap();
        assert!(!r.skip);
        assert_eq!(r.additional_flags, "-fno-inline");
        let r = eval(
            "if { ! [check_effective_target_nonlocal_goto] } {\n return 1\n}\nreturn 0;",
            &t,
            "",
        )
        .unwrap();
        assert!(!r.skip);
        let r = eval(
            "if { [check_effective_target_freestanding] } {\n return 1;\n}\nreturn 1\n",
            &t,
            "",
        )
        .unwrap();
        assert!(r.skip);
    }

    #[test]
    fn flags_and_xfail() {
        let t = arm64();
        let base = "-fno-tree-dse -fno-tracer";
        assert_eq!(
            eval("set additional_flags -fno-builtin-abs\nreturn 0\n", &t, base)
                .unwrap()
                .additional_flags,
            "-fno-builtin-abs"
        );
        assert_eq!(
            eval("lappend additional_flags \"-fno-ivopts\" \"-fno-gcse\"\nreturn 0\n", &t, base)
                .unwrap()
                .additional_flags,
            "-fno-tree-dse -fno-tracer -fno-ivopts -fno-gcse"
        );
        let x = "if { [istarget \"tic6x-*-*\"] && [check_effective_target_ti_c67x] } {\n\
                 \tset torture_execute_xfail \"tic6x-*-*\"\n}\n\
                 if { [istarget \"i?86-*-freebsd*\\[123\\]\\.*\"] } {\n\
                 \tset torture_execute_xfail \"i?86-*-freebsd*\"\n}\n\
                 if [istarget \"*-*-linux*\"] {\n    global target_triplet\n    set torture_execute_xfail \"$target_triplet\"\n}\nreturn 0\n";
        assert!(eval(x, &t, "").unwrap().xfail);
    }

    #[test]
    fn short_circuit_skips_unknown_commands() {
        let t = arm64();
        let x = "if { [istarget \"d10v-*-*\"] && ! [string-match \"*-mdouble64*\" $CFLAGS] } {\n\
                 \tset torture_execute_xfail \"d10v-*-*\"\n}\nreturn 0\n";
        assert_eq!(eval(x, &t, "").unwrap(), XResult::default());
        assert!(eval("frob 1\n", &t, "").is_err());
    }

    #[test]
    fn before_compile_continue() {
        let t = arm64();
        let x = "set torture_eval_before_compile {\n  if {[string match {*-flto*} \"$option\"]} {\n    continue\n  }\n}\nreturn 0\n";
        let r = eval(x, &t, "").unwrap();
        let body = r.before_compile.unwrap();
        assert!(option_allowed(&body, &t, "-O2").unwrap());
        assert!(!option_allowed(&body, &t, "-O2 -flto -fno-use-linker-plugin").unwrap());
    }

    #[test]
    fn else_branches_and_parens() {
        let t = arm64();
        let x = "if {([istarget \"x86_64-*-*\"] || [istarget \"aarch64*-*-*\"]) && 1 == 1} then {\n\
                 lappend additional_flags -DA\n} elseif [istarget \"avr-*-*\"] {\n return 1\n} else {\n return 1\n}\n";
        let r = eval(x, &t, "").unwrap();
        assert_eq!((r.skip, r.additional_flags.as_str()), (false, "-DA"));
    }
}
