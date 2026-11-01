// [class.mem.general]/8, [temp.inst]/3, [class.base.init]/9.1: a default
// member initializer that is invalid in its template's definition is
// diagnosed once there; using it (the implicit default constructor of an
// instantiated member class template, of a nested class of a class
// template, or of a class template) adds no further error: the
// initializer was parsed, it is not "used before it is parsed" (the
// WP-225 cascade, fixed by WP-223's instantiation on use).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:46 error "use of undeclared identifier 'nope'"
//@ diag: 18:22 error "use of undeclared identifier 'nope3'"
//@ diag: 22:37 error "use of undeclared identifier 'nope2'"
//@ diag: 24:57 error "use of undeclared identifier 'nope4'"

template <class T>
struct X {
  template <class U> struct M { U u = T(1) + nope; };
  struct N { int n = nope3; };
};
X<short>::M<int> m;
X<short>::N n;
template <class T> struct Y { T t = nope2; };
Y<int> y;
template <> template <class U> struct X<int>::M { U u = nope4; };
X<int>::M<long> ml;
