//! Error recovery and robustness: the lexer never panics, always makes
//! progress, and its spellings are stable ([lex.phases], [lex.pptoken]).
mod common;
use common::{cxx26, lex, std};
use kiln_lex::{Interner, LexOptions, Lexer, Std, TokKind, clean};
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
];

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
