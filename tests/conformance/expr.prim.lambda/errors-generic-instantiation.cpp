// [temp.inst]: an error in a generic lambda's body is diagnosed when the
// call operator specialization is instantiated; the note names it with
// its enclosing function and `(anonymous class)` for the closure type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:24 error "member reference base type 'int' is not a structure or union"
//@ diag: 14:11 note "in instantiation of function template specialization '(anonymous class)::operator()<int>' requested here"
//@ diag: 15:60 error "member reference base type 'int' is not a structure or union"
//@ diag: 15:72 note "in instantiation of function template specialization 'N::S::m()::(anonymous class)::operator()<int>' requested here"
//@ diag: 17:43 error "member reference base type 'int' is not a structure or union"
//@ diag: 16:35 note "in instantiation of function template specialization 'q(int, ...)::(anonymous class)::operator()<int>' requested here"
//@ diag: 17:54 note "in instantiation of function template specialization 'tf<(lambda at "
auto u = [](auto x) { x.foo(); };
int a = (u(1), 0);
namespace N { struct S { void m() { auto v = [](auto x) { x.foo(); }; v(2); } }; }
template <class T> void tf(T t) { t(3); }
void q(int, ...) { auto w = [](auto x) { x.foo(); }; tf(w); }
