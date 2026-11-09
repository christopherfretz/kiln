// GNU `__extension__` is a unary operator that the AST keeps (clang
// UO_Extension; WP-1029) but that is transparent everywhere else: the
// value, type and value category of its operand ([expr.unary] as a GNU
// extension), constant evaluation ([expr.const]), bit-field and
// overloaded-function operands, string-literal array initialization,
// case labels, template arguments, statement expressions and
// `__imag__` of an imaginary literal. `decltype(__extension__ g)` is
// `int &` because the operand is no longer an id-expression
// ([dcl.type.decltype]/1.3), which also changes its mangling (`DT`,
// tests/mangle/extension_decltype.cpp). Verified against clang 19.
//@ expect: run
//@ stdout: 33
extern "C" int printf(const char *, ...);
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
constexpr int a = __extension__ 1 + 2;
static_assert(a == 3);
constexpr int f() { int x = 1; __extension__ x = 2; __extension__ (x) += 3; return __extension__ x; }
static_assert(f() == 5);
char s[] = __extension__ "abc";
static_assert(sizeof(s) == 4);
static int g = 7;
static int *gp = __extension__ &g;
constexpr int *cgp = __extension__ &g;
struct B { int bf : 3; int m; };
int arr[__extension__ 3];
static_assert(sizeof(arr) == 3 * sizeof(int));
template <int N> struct A { static constexpr int v = N; };
static_assert(A<__extension__ 4>::v == 4);
int h(int x) { return x + 1; }
int h(double) { return 2; }
static_assert(same<decltype(__extension__ g), int &>);
static_assert(same<decltype((__extension__ g)), int &>);
static_assert(same<decltype(__extension__ 1), int>);
static_assert(noexcept(__extension__ g));
_Complex double cd = __extension__ 1.0i;
constexpr long long big = __extension__ 1LL << 62;
static_assert(big == 4611686018427387904LL);
int sw(int v) {
  switch (v) { case __extension__ 1: return 10; default: return 0; }
}
int run(int v) {
  B b{};
  __extension__ b.bf = 2;
  int r = __extension__ b.bf;
  int y = __extension__ ({ int t = v; t * 2; });
  int z = (__extension__ h)(1);
  constexpr int k = __extension__ f();
  return r + y + *gp + sw(1) + z + k + (int)__imag__ cd;
}
int main() { printf("%d\n", run(3)); }
