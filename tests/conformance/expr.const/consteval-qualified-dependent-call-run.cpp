// [expr.const]/18, [temp.res.general]/6, [temp.dep.expr]: in a template
// definition, a call of a consteval function whose arguments are
// type-dependent is a dependent call: its callee is not a reference to an
// immediate function outside an immediate invocation, whether it is named
// unqualified, by a qualified-id (`::mv`, `d1::mw`), through a
// using-declaration (`d2::mv`) or in parentheses. Each is checked as an
// immediate invocation when the template is instantiated. A qualified
// call with value-dependent arguments (q4) and a non-template call (n5)
// are the controls.
//@ expect: run
//@ strict-diags
//@ stdout: 2 2 2 2 3 2 2 4
extern "C" int printf(const char*, ...);

struct V { int a; };
consteval V mv(V x, V y) { return {x.a + y.a}; }
namespace d1 { consteval V mw(V x, V y) { return {x.a + y.a}; } }
namespace d2 { using ::mv; }
template <int H> consteval V wl() { return {H}; }

template <int H> int q1() { constexpr V v = ::mv(wl<H>(), V{1}); static_assert(v.a == H + 1); return v.a; }
template <int H> int q2() { constexpr V v = d1::mw(wl<H>(), V{1}); static_assert(v.a == H + 1); return v.a; }
template <int H> int q3() { constexpr V v = d2::mv(wl<H>(), V{1}); static_assert(v.a == H + 1); return v.a; }
template <int H> int q4() { constexpr V v = d2::mv(V{H}, V{1}); static_assert(v.a == H + 1); return v.a; }
int n5() { constexpr V v = d2::mv(V{2}, V{1}); static_assert(v.a == 3); return v.a; }
template <int H> int q6() { using d2::mv; constexpr V v = mv(wl<H>(), V{1}); static_assert(v.a == H + 1); return v.a; }
// Not a constexpr initializer: still an immediate invocation, evaluated.
template <int H> int q7() { V v = (::mv)(wl<H>(), V{1}); return v.a; }
template <class T> int q8() { V v = d1::mw(T{3}, V{1}); return v.a; }

int main() {
  printf("%d %d %d %d %d %d %d %d\n", q1<1>(), q2<1>(), q3<1>(), q4<1>(), n5(), q6<1>(),
         q7<1>(), q8<V>());
}
