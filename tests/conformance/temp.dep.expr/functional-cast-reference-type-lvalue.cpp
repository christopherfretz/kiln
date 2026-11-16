// [temp.dep.expr]/3: `R(x)` with `R` an lvalue reference type is an lvalue
// at the definition of the template, so it does not bind to an rvalue
// reference parameter.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 12:34 error "no matching function for call to 'take'"
//@ diag: 11:6 note "candidate function not viable: expects an rvalue for 1st argument"

typedef int& R;
void take(int&&);
template <class T> void g(T x) { take(R(x)); }
