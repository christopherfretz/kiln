// [over.match.class.deduct]: class template argument deduction from an
// initializer that already failed (an undeclared name, a variable whose
// own deduction failed) deduces nothing and adds no "no viable
// constructor or deduction guide" error (clang: an initializer that
// contains errors).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:5 error "use of undeclared identifier 'undeclared'"
//@ diag: 16:7 error "use of undeclared identifier 'nothing'"
//@ diag: 20:8 error "use of undeclared identifier 'absent'"
//@ diag: 21:21 error "use of undeclared identifier 'gone'"
template<class T> struct B { B(T); };
template<class T> struct P { T a, b; };
struct X { X(int); };
B b{undeclared};
X x = nothing;
B c{x};
B d = B(x);
B e(b);
P f{1, absent};
P g = {.a = 1, .b = gone};
