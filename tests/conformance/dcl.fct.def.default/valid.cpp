// [dcl.fct.def.default]/1-5: well-formed explicitly-defaulted special members,
// in class and out of line.
//@ expect: compile-ok
//@ strict-diags
struct A {
  A() = default;
  A(const A &) = default;
  A(A &&) = default;
  A &operator=(const A &) = default;
  A &operator=(A &&) = default;
  ~A() = default;
  int i;
};
struct B {
  B();
  B(const B &);
  B &operator=(const B &);
  ~B();
};
B::B() = default;
B::B(const B &) = default;
B &B::operator=(const B &) = default;
B::~B() = default;
struct C {
  constexpr C() = default;
  int i = 0;
};
