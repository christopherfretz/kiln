// [expr.type.conv]/1, [dcl.type.auto.deduct]/2.3 (C++23, P0849R8): the
// initializer of `auto(...)` / `auto{...}` is exactly one expression (not a
// braced list) whose type deduces `auto`; `decltype(auto)` is not a
// functional-cast type ([dcl.spec.auto.general]/5). Also checked when the
// operand is type-dependent. clang's wording.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 22:9 error "initializer for functional-style cast to 'auto' is empty"
//@ diag: 23:9 error "initializer for functional-style cast to 'auto' is empty"
//@ diag: 24:17 error "initializer for functional-style cast to 'auto' contains multiple expressions"
//@ diag: 25:17 error "initializer for functional-style cast to 'auto' contains multiple expressions"
//@ diag: 26:14 error "cannot deduce actual type for 'auto' from nested initializer list"
//@ diag: 27:14 error "cannot deduce actual type for 'auto' from parenthesized initializer list"
//@ diag: 28:9 error "'decltype(auto)' not allowed here"
//@ diag: 29:9 error "functional-style cast to 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 32:59 error "initializer for functional-style cast to 'auto' contains multiple expressions"
//@ diag: 34:56 error "cannot deduce actual type for 'auto' from nested initializer list"
void o(int);
void o(double);
void f(int i) {
  (void)auto();
  (void)auto{};
  (void)auto(1, 2);
  (void)auto{1, 2};
  (void)auto{{1}};
  (void)auto({1});
  (void)decltype(auto)(i);
  (void)auto(o);
}
template <class T>
void g(T t) { (void)auto{t}; (void)auto(t); (void)auto{t, t}; }
template <class T>
void h(T t) { (void)auto(t); (void)auto{t}; (void)auto{{t}}; }
