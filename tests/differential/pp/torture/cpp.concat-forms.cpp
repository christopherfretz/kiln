//@ expect: pp
// ## producing every kind of preprocessing token: identifiers, numbers
// (including exponents and digit separators), literals with prefixes and
// ud-suffixes, and multi-character punctuators.
#define CAT(a, b) a ## b
#define XCAT(a, b) CAT(a, b)
CAT(foo, bar) CAT(foo, 123) CAT(_, _) CAT(1, 2) CAT(1, e) CAT(1e, +)
CAT(0x, 1F) CAT(1'0, 00) CAT(1., 5f)
CAT(L, 'a') CAT(u8, "s") CAT(u, 'c') CAT(U, "s") CAT("s", _ud) CAT('c', _ud)
CAT(-, >) CAT(->, *) CAT(<, <=) CAT(<=, >) CAT(<<, =) CAT(>>, =) CAT(&, &)
CAT(|, |) CAT(+, +) CAT(-, -) CAT(:, :) CAT(., *) CAT(#, #) CAT(%:, %:)
CAT(!, =) CAT(=, =) CAT(^, =) CAT(%, =)
XCAT(XCAT(a, b), XCAT(c, d))
CAT(, x) CAT(x, ) CAT(, )
