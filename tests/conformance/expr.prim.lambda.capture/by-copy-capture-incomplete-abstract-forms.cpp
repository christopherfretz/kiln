// [expr.prim.lambda.capture]/10: an entity captured by copy becomes a
// non-static data member of the closure type, so its type must be complete
// ([basic.types.general]/5) and not an abstract class ([class.abstract]/3).
// clang captureInLambda: "by-copy capture of variable 'x' with incomplete
// type 'X'" (with the forward-declaration note) and "by-copy capture of
// value of abstract type 'A'" (the pure-virtual note once per class), at
// the explicit capture or at the use that captures implicitly; a nested
// lambda's use diagnoses the outermost failing capture only, and a lambda
// with a failed capture is invalid (no unused-value warning). A dependent
// type is checked when the template is instantiated.
// abstract class and an init-capture of one copy an abstract object
// ("allocating an object of abstract class type"). Found in downstream code.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:14 error "allocating an object of abstract class type 'A'"
//@ diag: 33:25 note "unimplemented pure virtual method 'f' in 'A'"
//@ diag: 39:15 error "by-copy capture of value of abstract type 'A'"
//@ diag: 40:26 error "by-copy capture of variable 'q' with incomplete type 'X'"
//@ diag: 32:8 note "forward declaration of 'X'"
//@ diag: 41:34 error "by-copy capture of value of abstract type 'A'"
//@ diag: 42:15 error "by-copy capture of value of abstract type 'A'"
//@ diag: 43:22 error "by-copy capture of value of abstract type 'A'"
//@ diag: 44:51 error "by-copy capture of variable 'x' with incomplete type 'X'"
//@ diag: 32:8 note "forward declaration of 'X'"
//@ diag: 47:45 error "by-copy capture of value of abstract type 'A'"
//@ diag: 48:15 note "in instantiation of function template specialization 'h<A>' requested here"
//@ diag: 49:68 error "by-copy capture of value of abstract type 'A'"
//@ diag: 49:60 note "while substituting into a lambda expression here"
//@ diag: 50:17 note "in instantiation of member function 'Tm<A>::m' requested here"
//@ diag: 53:55 error "allocating an object of abstract class type 'C'"
//@ diag: 53:25 note "unimplemented pure virtual method 'f' in 'C'"
struct X;
struct A { virtual void f() = 0; };
struct B : A {};
struct S { A &r; X &q; };
void g(X &x, A &a, S s) {
  auto l9 = [y = a] {};
  auto [r, q] = s;
  auto l10 = [r] {};
  auto l11 = [=] { (void)q; };
  auto l12 = [=](auto v) { (void)a; (void)v; };
  auto l13 = [a](auto) {};
  auto l14 = [&a] { [a] {}; };
  auto l15 = [&] { (void)[&] { return [=] { (void)x; }; }; };
  (void)l9; (void)l10; (void)l11; (void)l12; (void)l13; (void)l14; (void)l15;
}
template <class T> void h(T &t) { auto l = [t] {}; (void)l; }
template void h<A>(A &);
template <class T> struct Tm { void m(T &t) { auto l = [=] { (void)t; }; (void)l; } };
template struct Tm<A>;
template <class T> void ok3(T &t) { auto l = [t] {}; (void)l; }
template void ok3<int>(int &);
struct C { virtual void f() = 0; void m() { auto l = [*this] {}; (void)l; } };
struct D { void m() { auto l = [*this] {}; (void)l; } };
