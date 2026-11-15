// [expr.prim.lambda.capture]/10: an entity captured by copy becomes a
// non-static data member of the closure type, so its type must be complete
// ([basic.types.general]/5) and not an abstract class ([class.abstract]/3).
// clang captureInLambda: "by-copy capture of variable 'x' with incomplete
// type 'X'" (with the forward-declaration note) and "by-copy capture of
// value of abstract type 'A'" (the pure-virtual note once per class), at
// the explicit capture or at the use that captures implicitly; a nested
// lambda's use diagnoses the outermost failing capture only, and a lambda
// with a failed capture is invalid (no unused-value warning). A dependent
// type is checked when the template is instantiated. `[*this]` of an
// abstract class and an init-capture of one copy an abstract object
// ("allocating an object of abstract class type"). Found in downstream code.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:14 error "by-copy capture of variable 'x' with incomplete type 'X'"
//@ diag: 32:8 note "forward declaration of 'X'"
//@ diag: 38:14 error "by-copy capture of value of abstract type 'A'"
//@ diag: 33:25 note "unimplemented pure virtual method 'f' in 'A'"
//@ diag: 39:25 error "by-copy capture of value of abstract type 'A'"
//@ diag: 40:14 error "by-copy capture of value of abstract type 'B'"
//@ diag: 33:25 note "unimplemented pure virtual method 'f' in 'B'"
//@ diag: 41:14 error "by-copy capture of variable 'xs' with incomplete type 'X[2]'"
//@ diag: 32:8 note "forward declaration of 'X'"
//@ diag: 42:31 error "by-copy capture of value of abstract type 'A'"
//@ diag: 43:31 error "by-copy capture of value of abstract type 'A'"
//@ diag: 43:40 error "by-copy capture of variable 'x' with incomplete type 'X'"
//@ diag: 32:8 note "forward declaration of 'X'"
//@ diag: 44:14 error "by-copy capture of value of abstract type 'A'"
//@ diag: 44:17 error "by-copy capture of variable 'x' with incomplete type 'X'"
//@ diag: 32:8 note "forward declaration of 'X'"
//@ diag: 44:23 error "by-copy capture of value of abstract type 'A'"
struct X;
struct A { virtual void f() = 0; };
struct B : A {};
struct S { A &r; X &q; };
void g(X &x, A &a, B &b, X (&xs)[2]) {
  auto l1 = [x] {};
  auto l2 = [a] {};
  auto l3 = [=] { (void)a; };
  auto l4 = [b] {};
  auto l5 = [xs] {};
  auto l6 = [&] { [=] { (void)a; }; };
  auto l7 = [=] { [=] { (void)a; (void)x; }; };
  auto l8 = [a, x] { [a] {}; };
  auto ok1 = [&x, &a] {};
  auto ok2 = [&] { (void)x; (void)a; };
  (void)l1; (void)l2; (void)l3; (void)l4; (void)l5; (void)l6; (void)l7;
  (void)l8; (void)ok1; (void)ok2;
}
