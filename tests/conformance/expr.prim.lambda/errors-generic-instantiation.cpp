// [temp.inst]: an error in a generic lambda's body is diagnosed when the
// call operator specialization is instantiated; the note names it with
// its enclosing function and `(lambda)` for the closure type (clang 22).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:24 error "member reference base type 'int' is not a structure or union"
//@ diag: 16:11 note "in instantiation of function template specialization '(lambda)::operator()<int>' requested here"
//@ diag: 17:60 error "member reference base type 'int' is not a structure or union"
//@ diag: 17:72 note "in instantiation of function template specialization 'N::S::m()::(lambda)::operator()<int>' requested here"
//@ diag: 19:43 error "member reference base type 'int' is not a structure or union"
//@ diag: 18:35 note "in instantiation of function template specialization 'q(int, ...)::(lambda)::operator()<int>' requested here"
//@ diag: 19:54 note "in instantiation of function template specialization 'tf<(lambda at "
//@ requires: clang>=22
// clang 19 names the closure `(anonymous class)` in instantiation notes
auto u = [](auto x) { x.foo(); };
int a = (u(1), 0);
namespace N { struct S { void m() { auto v = [](auto x) { x.foo(); }; v(2); } }; }
template <class T> void tf(T t) { t(3); }
void q(int, ...) { auto w = [](auto x) { x.foo(); }; tf(w); }
