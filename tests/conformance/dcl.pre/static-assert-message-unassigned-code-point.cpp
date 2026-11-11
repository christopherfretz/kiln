// [dcl.pre]/12, [lex.string.uneval]: a failed static_assert's message prints
// as clang prints a string argument (llvm::sys::unicode::isPrintable, UCD
// 15.1.0, the version LLVM 22 generates its tables from): an unassigned code
// point (U+0378; U+1C89, assigned only in Unicode 16) and a private-use one
// escape as <U+XXXX>, while a printable non-ASCII character next to it
// (U+00E9, a Unicode 15.1 addition U+2FFC, an emoji, the last CJK Extension H
// ideograph U+31350, the variation selector U+E01EF), U+00AD, the format
// characters (U+200B, U+E0001) and a combining mark (U+0301) print raw.
// The second assertion holds the characters raw in the source.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion failed: <U+0378>é 😀 ⿼"
//@ diag: 17:15 error "static assertion failed: <U+0378>é ⿼"
//@ diag: 18:15 error "static assertion failed: <U+1C89><U+E000>­​́x"
//@ diag: 19:15 error "static assertion failed: 𱍐󠀁󠇯<U+E01F0>"
static_assert(false, "\u0378\u00E9 \U0001F600 \u2FFC");
static_assert(false, "͸é ⿼");
static_assert(false, "\u1C89\uE000\u00AD\u200B\u0301x");
static_assert(false, "\U00031350\U000E0001\U000E01EF\U000E01F0");
