// [dcl.fct]/3 (P3176R1): `...` directly after a function parameter pack
// (`Ts......`, `Ts...a...`) is a C varargs ellipsis, not part of the pack:
// clang's -Wambiguous-ellipsis, noting the pack's own `...` and the comma
// that silences it. A pack followed by `, ...` and a plain parameter
// followed by `...` are not ambiguous. C++23, so the C++26 deprecation of
// the comma-less ellipsis is not reported.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ diag: 25:36 warning "'...' in this location creates a C-style varargs function [-Wambiguous-ellipsis]"
//@ diag: 25:33 note "preceding '...' declares a function parameter pack"
//@ diag: 25:36 note "insert ',' before '...' to silence this warning"
//@ diag: 26:37 warning "'...' in this location creates a C-style varargs function [-Wambiguous-ellipsis]"
//@ diag: 26:33 note "preceding '...' declares a function parameter pack"
//@ diag: 26:37 note "insert ',' before '...' to silence this warning"
//@ diag: 27:47 warning "'...' in this location creates a C-style varargs function [-Wambiguous-ellipsis]"
//@ diag: 27:44 note "preceding '...' declares a function parameter pack"
//@ diag: 27:47 note "insert ',' before '...' to silence this warning"
//@ diag: 28:54 warning "'...' in this location creates a C-style varargs function [-Wambiguous-ellipsis]"
//@ diag: 28:51 note "preceding '...' declares a function parameter pack"
//@ diag: 28:54 note "insert ',' before '...' to silence this warning"
//@ diag: 29:44 warning "'...' in this location creates a C-style varargs function [-Wambiguous-ellipsis]"
//@ diag: 29:41 note "preceding '...' declares a function parameter pack"
//@ diag: 29:44 note "insert ',' before '...' to silence this warning"
template <class... Ts> void f(Ts......);
template <class... Ts> void g(Ts...a...);
template <class... Ts> struct X { void m(Ts......); };
template <class... Ts> void lam() { auto q = [](Ts......) {}; }
template <class... Ts> using F = void(Ts......);
template <class... Ts> void ok(Ts..., ...);
template <class T> void k(T...);
void l(int...);
int main() { f<int>(1, 2); g<int>(1, 2.0); }
