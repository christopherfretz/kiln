//! Error recovery and robustness: the lexer never panics, always makes
//! progress, and its spellings are stable ([lex.phases], [lex.pptoken]).
mod common;
use common::{cxx26, lex, std};
use kiln_lex::literal::{
    LitDiag, LiteralOptions, classify_number, decode_char, decode_string,
    decode_unevaluated_string, encode_string,
};
use kiln_lex::{Interner, LexOptions, LexSeverity, Lexer, Std, TokFlags, TokKind, clean};
use kiln_source::SourceManager;

/// [lex.pptoken]/2: garbage still produces tokens and diagnostics.
#[test]
fn lex_recovery_garbage() {
    let l = lex("\u{1}\u{7f}#\u{2603}\\\\'\"\n");
    assert_ne!(l.tokens.len(), 0);
    assert_ne!(l.diags.len(), 0);
}

/// [lex.phases]: Eof is sticky.
#[test]
fn lex_recovery_eof_sticky() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("<e>", "a /* x".to_owned()).unwrap();
    let i = Interner::new();
    let mut lx = Lexer::new(&sm, &i, f, sm.loc(f, 0), cxx26());
    let t = lx.next();
    assert_eq!(lx.spelling(&t), "a");
    for _ in 0..3 {
        assert!(lx.next().is_eof());
    }
    assert_eq!(lx.take_diagnostics().len(), 2); // unterminated comment, no newline at EOF
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const PIECES: &[&str] = &[
    "\\",
    "\n",
    "\r",
    "\r\n",
    " ",
    "\t",
    "?",
    "??",
    "/",
    "*",
    "\"",
    "'",
    "R",
    "u8",
    "L",
    "u",
    "(",
    ")",
    "a",
    "_x",
    "0",
    "1",
    ".",
    "e",
    "p",
    "+",
    "-",
    "<",
    ">",
    ":",
    "%",
    "[",
    "]",
    "^",
    "=",
    "#",
    "{",
    "}",
    "\u{e9}",
    "\u{300}",
    "\u{3000}",
    "\u{2603}",
    "\0",
    "\\u00e9",
    "\\U0001F600",
    "\\u{",
    "\\N{",
    "$",
    "@",
    "d(",
    ")d\"",
    "\\x",
    "\\x{",
    "\\o{",
    "\\377",
    "\\n",
    "\\N{LATIN SMALL LETTER A}",
    "\\N{latin small letter a}",
    "f",
    "ull",
    "'0",
    "0x",
    "0b",
    "_s",
];

/// The literal module on one spelling: no panic, diagnostic ranges inside
/// the spelling, `has_error` consistent, and (for an error-free string)
/// `encode_string` round-trips the value.
fn check_literal(sp: &str, kind: Option<TokKind>, std: Std) {
    let opts = LiteralOptions { std, ..LiteralOptions::default() };
    let ok = |diags: &[LitDiag], has_error: bool| {
        for d in diags {
            assert!(d.range.start <= d.range.end, "range on {sp:?}: {d:?}");
            assert!(d.range.end as usize <= sp.len(), "range past end on {sp:?}: {d:?}");
        }
        let any = diags.iter().any(|d| d.kind.severity() == LexSeverity::Error);
        assert_eq!(any, has_error, "has_error on {sp:?}: {diags:?}");
    };
    let string = matches!(kind, None | Some(TokKind::StringLit(_) | TokKind::RawStringLit(_)));
    if string {
        let s = decode_string(sp, opts);
        ok(&s.diags, s.has_error);
        if !s.has_error && s.ud_suffix.is_none() && !s.raw {
            let enc = encode_string(&s.units, s.encoding);
            assert_eq!(decode_string(&enc, opts).units, s.units, "round trip of {sp:?}");
        }
        let u = decode_unevaluated_string(sp, opts);
        ok(&u.diags, u.has_error);
    }
    if matches!(kind, None | Some(TokKind::CharLit(_))) {
        let c = decode_char(sp, opts);
        ok(&c.diags, c.has_error);
    }
    if matches!(kind, None | Some(TokKind::PpNumber)) {
        let n = classify_number(sp, std);
        ok(&n.diags, n.has_error);
        if let kiln_lex::literal::NumberClass::Floating(f) = &n.class {
            let _ = (f.parse_f64(), f.parse_f32());
        }
        if let kiln_lex::literal::NumberClass::Integer(i) = &n.class {
            let _ = i.value();
        }
    }
}

fn check(text: &str, opts: LexOptions) {
    let sm = SourceManager::new();
    let f = sm.add_virtual("<fuzz>", text.to_owned()).unwrap();
    let i = Interner::new();
    let mut lx = Lexer::new(&sm, &i, f, sm.loc(f, 0), opts);
    let mut prev_hi = 0u32;
    // Every non-Eod token consumes at least one byte.
    for n in 0..=text.len() + 2 {
        let t = lx.next();
        if t.is_eof() {
            assert!(lx.next().is_eof());
            return;
        }
        assert!(n <= 2 * text.len() + 2, "no progress on {text:?}");
        let (_, lo) = sm.offset_of(t.span.lo).unwrap();
        let (_, hi) = sm.offset_of(t.span.hi).unwrap();
        assert!(lo >= prev_hi && hi >= lo, "spans out of order on {text:?}");
        if t.kind != TokKind::Eod {
            assert!(hi > lo, "empty token on {text:?}");
        }
        prev_hi = hi;
        let sp = lx.spelling(&t);
        // Phase 2 is idempotent. (Phase 1 trigraph replacement is not: a
        // splice can bring `??` and `-` together, which must not form a
        // trigraph, so re-cleaning never translates trigraphs.)
        assert_eq!(clean(&sp, t.kind, false), sp, "spelling not idempotent on {text:?}");
        let raw = &text[lo as usize..hi as usize];
        assert_eq!(clean(raw, t.kind, opts.trigraphs), sp);
        // The lexer's skip-the-scan cursor (WP-056) never hides a splice.
        assert_eq!(
            t.has(TokFlags::NEEDS_CLEANING),
            clean(raw, t.kind, opts.trigraphs) != raw,
            "NEEDS_CLEANING of {raw:?} in {text:?}"
        );
        if matches!(
            t.kind,
            TokKind::StringLit(_)
                | TokKind::RawStringLit(_)
                | TokKind::CharLit(_)
                | TokKind::PpNumber
        ) {
            check_literal(&sp, Some(t.kind), opts.std);
        }
    }
    // Eod tokens can interleave, but never more than one per byte.
    panic!("lexer did not terminate on {text:?}");
}

/// [lex.phases], [lex.pptoken]: bounded random-input fuzz. No panics,
/// termination, ordered spans, and idempotent phase-2 cleaning.
#[test]
fn lex_fuzz_random_bytes() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let opts = [
        cxx26(),
        std(Std::Cxx98),
        std(Std::Cxx17),
        LexOptions { trigraphs: true, keep_comments: true, ..std(Std::Cxx11) },
        LexOptions { dollar_in_identifiers: false, ..cxx26() },
    ];
    // `KILN_LEX_FUZZ_ITERS` raises the bound for longer local runs.
    let iters =
        std::env::var("KILN_LEX_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(3000);
    let seed = std::env::var("KILN_LEX_FUZZ_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let max_len =
        std::env::var("KILN_LEX_FUZZ_LEN").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    rng.0 ^= seed;
    for iter in 0..iters {
        let len = rng.below(max_len);
        let text = if iter % 3 == 0 {
            let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            (0..len).map(|_| PIECES[rng.below(PIECES.len())]).collect()
        };
        check(&text, opts[iter % opts.len()]);
    }
}

/// [lex.phases]/1.2: long mostly-clean inputs with rare splices and
/// trigraphs, so that the lexer's clean-run cursor is reused across many
/// tokens; every token's spelling and `NEEDS_CLEANING` stay exact.
#[test]
fn lex_phases_sparse_splices_fuzz() {
    const CLEAN: &[&str] =
        &["ab", " ", "\n", "12", "+", "/* c */", "\"s\"", "'c'", "R\"(x)\"", "_y"];
    const DIRTY: &[&str] = &["\\\n", "\\ \n", "??/\n", "??=", "?", "\\", "R\"(\\\n)\""];
    let mut rng = Rng(0x5851_F42D_4C95_7F2D);
    for iter in 0..300 {
        let len = 50 + rng.below(400);
        let text: String = (0..len)
            .map(|_| {
                if rng.below(25) == 0 {
                    DIRTY[rng.below(DIRTY.len())]
                } else {
                    CLEAN[rng.below(CLEAN.len())]
                }
            })
            .collect();
        let trigraphs = iter % 2 == 0;
        check(&text, LexOptions { trigraphs, ..std(Std::Cxx11) });
    }
}

/// [lex.ccon], [lex.string], [lex.icon], [lex.fcon]: the literal decoders
/// take arbitrary text (not only lexer-made spellings) without panicking.
#[test]
fn lex_fuzz_literal_decoders() {
    const HEADS: &[&str] =
        &["\"", "'", "u8\"", "L'", "U\"", "u'", "R\"", "u8R\"x(", "0x", "1", ".", ""];
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let iters =
        std::env::var("KILN_LEX_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(3000);
    for iter in 0..iters {
        let len = rng.below(16);
        let mut text: String = HEADS[rng.below(HEADS.len())].to_owned();
        if iter % 4 == 0 {
            let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            text.push_str(&String::from_utf8_lossy(&bytes));
        } else {
            text.extend((0..len).map(|_| PIECES[rng.below(PIECES.len())]));
        }
        if rng.below(2) == 0 {
            text.push_str(["\"", "'", ")x\"", "\"_s", "'_x"][rng.below(5)]);
        }
        let std = [Std::Cxx98, Std::Cxx11, Std::Cxx17, Std::Cxx26][iter % 4];
        check_literal(&text, None, std);
    }
}

/// The fuzz checker also drives directive mode: Eod at every newline.
#[test]
fn lex_fuzz_directive_mode() {
    let mut rng = Rng(42);
    for _ in 0..1000 {
        let len = rng.below(30);
        let text: String = (0..len).map(|_| PIECES[rng.below(PIECES.len())]).collect();
        let sm = SourceManager::new();
        let f = sm.add_virtual("<fuzz>", text.clone()).unwrap();
        let i = Interner::new();
        let mut lx = Lexer::new(&sm, &i, f, sm.loc(f, 0), cxx26());
        let mut steps = 0;
        let mut last_eod = false;
        loop {
            // As kiln_pp does: (re-)enter a directive, but not right after
            // its Eod at end of file.
            if !last_eod {
                lx.set_in_directive(true);
            }
            let t = lx.next();
            if t.is_eof() {
                break;
            }
            last_eod = t.kind == TokKind::Eod;
            steps += 1;
            assert!(steps <= 2 * text.len() + 2, "no progress on {text:?}");
        }
    }
}
