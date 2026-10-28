//@ expect: pp
// The -E printer must insert a space wherever two adjacent output tokens
// would otherwise lex as a different token sequence. Each line places
// tokens from different expansions next to each other.
#define EMPTY
#define PLUS +
#define ID(x) x
+EMPTY+ -EMPTY- -EMPTY> &EMPTY& |EMPTY| <EMPTY< >EMPTY> =EMPTY=
.EMPTY.EMPTY. /EMPTY/ /EMPTY* <EMPTY: %EMPTY: :EMPTY: <EMPTY%
PLUS+ PLUS PLUS +PLUS ID(+)+ ID(+)ID(+) ID(-)> ID(<)ID(<)ID(=)
L EMPTY"x" u8 EMPTY"y" x EMPTY y 1 EMPTY 2 1 EMPTY.2 1 EMPTY e EMPTY+1
#define N 1
#define E e
N E N.E 0x E ID(1)ID(2) ID(a)ID(b) ID(.)ID(5)
#define HASH #
#define HH # ## #
HASH HASH HH ID(#)ID(#)
