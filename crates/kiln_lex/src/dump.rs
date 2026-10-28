//! A textual token dump for tests and debugging (`kind 'spelling' flags`).

use std::fmt::Write as _;

use kiln_source::SourceManager;

use crate::interner::Interner;
use crate::lexer::Lexer;
use crate::options::LexOptions;
use crate::spelling::spelling;
use crate::token::{TokKind, Token};

/// One line describing `tok`: `kind 'spelling'` followed by its flags as
/// ` [StartOfLine] [LeadingSpace] [UnClean] [Digraph] [UCN] [UDSuffix]`
/// (the set ones, in that order). Kind names follow clang's
/// `-dump-tokens` where clang has the kind (see [`TokKind::name`]). In the
/// spelling, newline, carriage return and tab are written as `\n`, `\r`
/// and `\t`; everything else (including `\`, so UCNs read naturally) is
/// written as is.
pub fn dump_token(sm: &SourceManager, opts: &LexOptions, tok: &Token) -> String {
    let mut out = String::new();
    write_token(&mut out, sm, *opts, tok);
    out
}

/// [`dump_token`] for each token, one per line (each line ends in `\n`).
pub fn dump(sm: &SourceManager, opts: &LexOptions, toks: &[Token]) -> String {
    let mut out = String::new();
    for t in toks {
        write_token(&mut out, sm, *opts, t);
        out.push('\n');
    }
    out
}

/// Lexes `text` as a whole file and dumps every token up to and including
/// `Eof`. Convenience for tests and tools.
///
/// # Panics
/// If `text` is too large for a [`SourceManager`] buffer.
pub fn dump_source(text: &str, opts: LexOptions) -> String {
    let sm = SourceManager::new();
    let file = sm.add_virtual("<dump>", text.to_owned()).expect("source too large");
    let interner = Interner::new();
    let mut lx = Lexer::new(&sm, &interner, file, sm.loc(file, 0), opts);
    let mut toks = Vec::new();
    loop {
        let t = lx.next();
        toks.push(t);
        if t.kind == TokKind::Eof {
            break;
        }
    }
    dump(&sm, &opts, &toks)
}

fn write_token(out: &mut String, sm: &SourceManager, opts: LexOptions, tok: &Token) {
    out.push_str(&tok.kind.name());
    out.push_str(" '");
    for c in spelling(sm, tok, &opts).chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('\'');
    tok.flags.for_each_name(|n| {
        let _ = write!(out, " [{n}]");
    });
}
