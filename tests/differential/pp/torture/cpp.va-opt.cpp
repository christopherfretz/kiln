//@ expect: pp
// [cpp.subst]: __VA_OPT__ expands to its content only when the variable
// arguments are non-empty after macro replacement.
#define F(...)           f(0 __VA_OPT__(,) __VA_ARGS__)
#define G(X, ...)        f(0, X __VA_OPT__(,) __VA_ARGS__)
#define SDEF(sname, ...) S sname __VA_OPT__(= { __VA_ARGS__ })
#define EMP

F(a, b, c) F() F(EMP)
G(a, b, c) G(a, ) G(a)
SDEF(foo); SDEF(bar, 1, 2);

// __VA_OPT__ is a placemarker when empty, and its content can be pasted.
#define H2(X, Y, ...) __VA_OPT__(X ## Y,) __VA_ARGS__
H2(a, b, c, d)
#define H3(X, ...) #__VA_OPT__(X##X X##X)
H3(, 0)
#define H4(X, ...) __VA_OPT__(a X ## X) ## b
H4(, 1)
#define H5A(...) __VA_OPT__()/**/__VA_OPT__()
#define H5B(X) a ## X ## b
#define H5C(X) H5B(X)
H5C(H5A())

// __VA_OPT__ content is rescanned with the rest of the replacement.
#define LPAREN() (
#define GQ(Q) 42
#define FR(R, X, ...) __VA_OPT__(GQ R X) )
int x = FR(LPAREN(), 0, <:-);

// Nested in other replacements, and with an argument that expands to a comma.
#define COMMA ,
#define V(...) [__VA_OPT__(<__VA_ARGS__>)]
V(COMMA) V(()) V(EMP EMP) V(/* nothing */)
