// [basic.scope.scope]/4, [namespace.def.general]/6, [namespace.memdef]/2:
// a member of an inline namespace is found by lookup in the enclosing
// namespace, but it is declared in another scope, so an unqualified
// declaration in the enclosing namespace declares a distinct entity
// (clang's FilterLookupForScope without AllowInlineNamespace). A
// qualified declaration and an explicit specialization may still name
// the inline namespace's member ([temp.expl.spec]/3).
namespace N {
inline namespace I {
int f;
typedef int T;
enum E { e };
template <class U> struct C { static constexpr int v = 1; };
void g(int);
} // namespace I
void f();
typedef long T;
int e;
template <class U> struct C { static constexpr int v = 2; };
void g(int);
} // namespace N
static_assert(N::I::C<int>::v == 1);
static_assert(sizeof(N::I::T) == sizeof(int));

namespace A {
inline namespace I {
template <class T> constexpr int f(T) { return 0; }
template <class T> struct S { static constexpr int v = 0; };
extern int x;
} // namespace I
template <> constexpr int f<int>(int) { return 1; }
template <> struct S<int> { static constexpr int v = 1; };
int I::x = 3;
using I::x;
} // namespace A
static_assert(A::f(0) == 1 && A::f('a') == 0);
static_assert(A::S<int>::v == 1 && A::S<char>::v == 0);
int read_x() { return A::x; }
