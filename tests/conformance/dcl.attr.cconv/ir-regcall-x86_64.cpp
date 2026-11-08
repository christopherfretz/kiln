// [dcl.fct] [dcl.attr.grammar]/5 An x86-64 `regcall` function classifies
// its arguments and result as clang 19 (`X86_64ABIInfo::
// classifyRegCallStructType`): records flattened into up to 16 integer
// and 16 SSE registers, a non-trivial or over-budget one indirect,
// `long double` and complex `long double` in x87 registers, and the
// `__regcall3__` name prefix. spec/types.md §5.4, WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
struct Q { int a; double b; };
struct NT { int a; NT(const NT&); };
struct In { int x, y; };
struct Ne { In i; float f; };
struct Ar { int a[3]; };
struct LD { long double d; };
union U { int i; float f; };
struct Big { long a, b, c, d, e, f, g, h, i, j, k, l; };
struct E {};
#define R __attribute__((regcall))
R Q f1(Q, NT, Ne, Ar) { return {}; }
R void f2(LD, U, E) {}
R void f3(Big) {}
R Big f4() { return {}; }
R _Complex long double f5() { return 0; }
R void f6(long, long, long, long, long, long, long, long, long, long, long, Q) {}
R In f7(In) { return {}; }
