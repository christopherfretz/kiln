//! [cpp.cond]/13 Randomized skipped groups: the skipping fast paths (WP-056)
//! never hide or invent a directive.
//!
//! Each case nests random conditional groups (with random directive tails
//! and random lines of comments, literals, raw strings, splices, digit
//! separators and stray quotes) inside `#if 0`, then `#else` / `#endif`
//! around marker tokens. Whatever the skipped text, exactly the markers
//! come out, with no diagnostics. A fast path that skipped into a comment,
//! a literal or a splice would desynchronize the group structure and lose
//! or duplicate a marker.

mod common;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }
}

/// Pieces of one logical line. None ends the line or leaves a comment,
/// raw string or splice open past it, and none glues to its neighbours
/// into an unterminated literal or a comment opener.
const PIECES: &[&str] = &[
    "a",
    "_b",
    " ",
    "\t",
    "12",
    " 1'0",
    "+",
    "#",
    "%:",
    "(",
    ")",
    ",",
    ";",
    "<x>",
    " \"s\"",
    " 'c'",
    " /* c */",
    " /*\n#endif\n*/",
    " R\"(r)\"",
    " R\"x(\n#endif\n)x\"",
    "\\\nq",
    "\\ \nq",
    "\\x",
    "?",
    "??",
    "?\\\n?",
    "/ ",
    "* ",
    "\u{e9}",
    "x\\\n#endif ",
    "x\\ \n# else ",
];

/// Line endings that swallow the rest of the line: a stray quote or a
/// line comment (which a splice continues onto the next physical line).
const LAST: &[&str] = &["", "", "", " '", " \"", " '/*", " \"/*", " // c", " // c \\\n#endif"];

fn tail(rng: &mut Rng, out: &mut String) {
    for _ in 0..rng.below(8) {
        out.push_str(PIECES[rng.below(PIECES.len())]);
    }
    out.push_str(LAST[rng.below(LAST.len())]);
    out.push('\n');
}

/// A skipped group's text; every `#if` it opens is closed.
fn group(rng: &mut Rng, depth: u32, out: &mut String) {
    for _ in 0..rng.below(6) {
        match rng.below(10) {
            0..=4 => tail(rng, out),
            5 if depth < 3 => {
                out.push_str(["#if ", "#ifdef ", "#ifndef ", " # if "][rng.below(4)]);
                tail(rng, out);
                group(rng, depth + 1, out);
                for _ in 0..rng.below(3) {
                    out.push_str("#elif ");
                    tail(rng, out);
                    group(rng, depth + 1, out);
                }
                if rng.below(2) == 0 {
                    out.push_str("#else ");
                    tail(rng, out);
                    group(rng, depth + 1, out);
                }
                out.push_str("#endif ");
                tail(rng, out);
            }
            _ => {
                let d = ["#define", "#undef", "#pragma", "#include", "#error", "#", "#line", "#x"];
                out.push_str(d[rng.below(d.len())]);
                out.push(' ');
                tail(rng, out);
            }
        }
    }
}

/// [cpp.cond]/13
#[test]
fn cpp_cond_random_skipped_groups() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..600 {
        let mut src = String::from("#if 0\n");
        group(&mut rng, 0, &mut src);
        src.push_str("#else\nIN\n#endif\nOUT\n#if 1\nIN2\n#else\n");
        group(&mut rng, 0, &mut src);
        src.push_str("#endif\nOUT2\n");
        let o = common::pp(&src);
        assert_eq!(
            o.text.split_whitespace().collect::<Vec<_>>(),
            ["IN", "OUT", "IN2", "OUT2"],
            "output of:\n{src}"
        );
        assert!(o.diags.is_empty(), "diagnostics for:\n{src}\n{:#?}", o.diags);
    }
}
