// clang's `annotate` attribute on namespace-scope variables and functions
// (WP-1113, [dcl.attr.grammar]/5, [basic.def]/2): one
// `@llvm.global.annotations` element per attribute, in attribute order
// (an inherited one ahead of the declaration's own, one whose string the
// later declaration repeats replaced by it), with the annotation string
// and file name as `.str` constants in `llvm.metadata` shared by contents,
// the line of the declaration, and the other arguments as a `.args` struct
// shared between equal values: `1`, `1u`, `true` and `(char)1` share one,
// `1L` does not, and pointers into two string literals do not. Variables
// are annotated when defined (a declaration-only one or one annotated
// after its definition is not), functions at the end of the module by
// their latest declaration.
int g;
enum E { E1 = 5 };
struct P { int a; double b; };
void ff();
int g1 [[clang::annotate("glob")]] = 1;
int g2 [[clang::annotate("glob", 1, 2.5, "s")]] = 2;
int g3 [[clang::annotate("glob", 1, 2.5, "s")]] [[clang::annotate("two")]] = 3;
int a1 [[clang::annotate("x", 1)]];
int a2 [[clang::annotate("x", 1)]];
int a3 [[clang::annotate("x", 1L)]];
int a4 [[clang::annotate("x", 1u)]];
int a5 [[clang::annotate("x", true)]];
int a6 [[clang::annotate("x", (char)1)]];
int a7 [[clang::annotate("x", 1, 0)]];
int a8 [[clang::annotate("x", &g)]];
int a9 [[clang::annotate("x", &g)]];
int a10 [[clang::annotate("x", 1.5f, 'c', true, E1, nullptr, ff)]];
int a11 [[clang::annotate("x", P{1, 2.0})]];
int a12 [[clang::annotate("x", (void*)0)]];
int a13 [[clang::annotate("annotate_global.cpp")]];
int a14 [[clang::annotate("x", u8"s", L"w")]];
int a15 [[clang::annotate("x", 1.0)]];
int a16 [[clang::annotate("x", 1.0f)]];
constexpr int cx = 9;
int a17 [[clang::annotate("x", cx + 1, sizeof(P))]];
const char* p = "s";
[[clang::annotate("v")]] extern int v1;
[[clang::annotate("w")]] int v1 = 1;
extern int v2 [[clang::annotate("v2")]];
int v2 = 2;
int v3 = 3;
extern int v3 [[clang::annotate("v3")]];
extern int decl_only [[clang::annotate("decl")]];
int useit() { return decl_only + v3; }
[[clang::annotate("fn", &g1)]] int f(int x) { return x; }
[[clang::annotate("declfn")]] void declfn();
void callit() { declfn(); }
void decl_then_def();
void use1() { decl_then_def(); }
[[clang::annotate("dtd")]] void decl_then_def() {}
[[clang::annotate("a")]] void f1();
void u1() { f1(); }
[[clang::annotate("b")]] void f1();
void u2() { f1(); }
[[clang::annotate("c")]] void f2() {}
[[clang::annotate("d")]] void f2();
void u3() { f2(); }
[[clang::annotate("e")]] [[clang::annotate("e", 1)]] void f3();
[[clang::annotate("e", 2)]] void f3() {}
extern "C" [[clang::annotate("cfn")]] int cfn(int x) { return x + 1; }
static int unused_static [[clang::annotate("dropped")]];
#line 100 "other.cpp"
int a18 [[clang::annotate("line")]];
