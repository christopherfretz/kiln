// [namespace.def.general]/3 mangling (WP-315, spec/sema.md §4.6): a
// namespace definition that extends a namespace of the enclosing
// namespace's inline namespace set mangles under the namespace it is
// written in, as clang does: `A::R::y` declared in `namespace A {
// namespace R {` (reopening `A::I::R`) is `_ZN1A1R1yE`. An entity is
// mangled through the namespace definition of its first declaration;
// specializations live in their template's namespace.
namespace A {
inline namespace I {
namespace R {
int x = 1;
int f();
struct S { int g(); };
template <class T> int h(T) { return 0; }
template <class T> struct TS { static int v; };
}
}
int use() { return R::f(); }
namespace R {
int y = x + 1;
int f() { return 2; }
int S::g() { return 3; }
struct U { int k() { return 4; } };
int w(U u) { return u.k() + h(0); }
template <> int h(long) { return 5; }
template <class T> int TS<T>::v = 1;
namespace Deep { int d = 0; }
}
}
namespace A::R {
int z = y + 1;
int q() { return S().g() + h(1L) + TS<int>::v + Deep::d + w(U()); }
}
int main() { return A::R::q() + A::use() + A::R::z; }
