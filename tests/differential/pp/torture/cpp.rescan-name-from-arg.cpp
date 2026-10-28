//@ expect: pp
// A function-like macro name produced by an argument (or by the
// replacement list) is invoked when rescanning finds a following `(`,
// including a `(` that comes from the rest of the source file.
#define F(x) [x]
#define APPLY(m, a) m(a)
#define NAME F
#define LATE NAME
APPLY(F, 1)
APPLY(NAME, 2)
LATE(3)
NAME
(4)
#define G() NAME
G()(5)
#define H(x) x(6)
H(F) H(NAME)
#define LPAREN (
F LPAREN 7)
#define EMPTY
F EMPTY (8)
