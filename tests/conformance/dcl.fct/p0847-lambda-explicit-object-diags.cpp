// [expr.prim.lambda.closure]/5, [dcl.fct]/6 (C++23, P0847R7): an explicit
// object parameter of a lambda is its first parameter, has no default
// argument and is no pack; the lambda is neither `mutable` nor `static`.
// With a lambda-capture, the object parameter's type is the closure type
// or a class publicly derived from it: checked where a non-template call
// operator is declared, and at each call of a generic one.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 40:15 error "a lambda with an explicit object parameter cannot be mutable"
//@ diag: 41:15 error "an explicit object parameter cannot appear in a static lambda"
//@ diag: 42:30 error "an explicit object parameter can only appear as the first parameter of the lambda"
//@ diag: 43:25 error "the explicit object parameter cannot have a default argument"
//@ diag: 44:15 error "the explicit object parameter cannot be a function parameter pack"
//@ diag: 50:3 error ">' in lambda with capture; the type must derive publicly from the lambda"
//@ diag: 23:34 note "declared private here"
//@ diag: 52:3 error ">' in lambda with capture; the type must derive publicly from the lambda"
//@ diag: 27:34 note "declared protected here"
//@ diag: 54:23 error "invalid explicit object parameter type 'C' in lambda with capture; the type must be the same as, or derived from, the lambda"
//@ diag: 61:3 error "no matching function for call to object of type '(lambda at "
//@ diag: 60:12 note "to 'int' for object argument"
//@ diag: 60:12 note "conversion candidate of type 'void (*)(int)'"
template <class L> struct Priv : private L {
  Priv(L l) : L(l) {}
  using L::operator();
};
template <class L> struct Prot : protected L {
  Prot(L l) : L(l) {}
  using L::operator();
};
template <class L> struct Pub : L {
  Pub(L l) : L(l) {}
};
struct C {
  template <class L> C(L) {}
  int v = 4;
};

void specifiers() {
  auto a = [](this auto self) mutable {};
  auto b = [](this auto self) static {};
  auto c = [](int, this auto self) {};
  auto d = [](this auto = 0) {};
  auto e = [](this auto... s) {};
}

void object_type() {
  int x = 1;
  auto e = [=](this auto self) { return 1; };
  Priv{e}();
  auto g = [x](this auto self) { return x; };
  Prot{g}();
  Pub{g}();
  auto h = [x](this C self) { return self.v; };
  h();
  auto k = [](this C self) { return self.v; };
  k();
  auto n = [](this auto self) { return 1; };
  Priv{n}();
  auto m = [](this int) {};
  m();
}
