// [dcl.attr.nodiscard]/4, [stmt.expr] (WP-461): clang DiagnoseUnusedExprResult
// looks through an explicit cast whose kind is NoOp, ConstructorConversion
// or IntegralCast (its implicit casts stripped). A conversion function, a
// call or a construction under the cast is checked for nodiscard, pure and
// const, and reported at the cast. An overloaded comparison that is not the
// statement itself (`(a == b);`, a conditional's arm) is checked as the
// call it is, at its operator.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 58:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 59:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 61:3 warning "ignoring return value of function declared with 'nodiscard' attribute: msg"
//@ diag: 62:3 warning "declared with 'nodiscard' attribute"
//@ diag: 63:3 warning "ignoring return value of function declared with pure attribute"
//@ diag: 64:3 warning "ignoring return value of function declared with pure attribute"
//@ diag: 67:3 warning "declared with 'nodiscard' attribute"
//@ diag: 68:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute"
//@ diag: 70:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 71:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 72:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 75:6 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 76:5 warning "equality comparison result unused"
//@ diag: 77:6 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 79:7 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 81:21 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 60:3 warning "expression result unused"
//@ diag: 66:3 warning "expression result unused"
//@ diag: 69:3 warning "expression result unused"
//@ diag: 73:3 warning "expression result unused"
//@ diag: 78:6 warning "expression result unused"
//@ diag: 80:6 warning "expression result unused"
//@ diag: 51:37 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 74:3 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 51:56 warning "ignoring return value of function declared with 'nodiscard' attribute"
struct S {
  [[nodiscard]] explicit operator bool() const;
  [[nodiscard("msg")]] operator char() const;
};
struct [[nodiscard]] N {};
struct T {
  operator N() const;
  operator long() const __attribute__((pure));
};
struct [[nodiscard]] A { A(int); };
struct B { [[nodiscard]] B(int); };
struct C { C(int); };
[[nodiscard]] const int &r();
[[nodiscard]] int v();
int k() __attribute__((const));
template <class U> struct W { [[nodiscard]] operator U() const; };
template <class U> void g(W<U> w) { static_cast<U>(w); (U)w; }
struct I {
  [[nodiscard]] friend bool operator==(I, I) { return true; }
  [[nodiscard]] friend bool operator!=(I, int) { return true; }
};
struct J { int v; friend bool operator==(J, J) = default; };
void f(S s, T t, I a, I b, J c, bool p) {
  static_cast<bool>(s);
  (bool)s;
  bool{s};
  static_cast<char>(s);
  static_cast<N>(t);
  static_cast<long>(t);
  static_cast<int>(t);
  (void)static_cast<bool>(s);
  const_cast<const S &>(s);
  static_cast<A>(1);
  static_cast<B>(1);
  static_cast<C>(1);
  const_cast<int &>(r());
  static_cast<long>(v());
  (long)k();
  static_cast<const int &>(v());
  g(W<int>{});
  (a == b);
  a == b;
  (a != 1);
  (c == c);
  ((a == b));
  (a != b);
  p ? (a == b) : (a == b);
}
