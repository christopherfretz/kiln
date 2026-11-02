// [dcl.decl.general]/4, [dcl.attr.grammar]/5, [dcl.attr.nodiscard]/4,
// [dcl.attr.deprecated]: an attribute-specifier-seq after the declarator-id
// appertains to the declared entity whatever the declarator-id is (an
// identifier, an operator-function-id, a member template, a friend), so
// `[[nodiscard]]` and `[[deprecated]]` written there take effect.
// Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 30:4 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 31:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 32:4 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 33:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 34:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 35:5 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 36:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 37:3 warning "'f' is deprecated: old"
//@ diag: 27:9 note "'f' has been explicitly marked deprecated here"
//@ diag: 38:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
struct S {
  int operator() [[nodiscard]] (int x) const { return x; }
  template<typename T> T g [[nodiscard]] (T t) const { return t; }
  int operator[] [[nodiscard]] (int i) const { return i; }
  friend bool operator== [[nodiscard]] (S, S) { return true; }
  friend int fr [[nodiscard]] (S) { return 0; }
  S operator+ [[nodiscard]] (S) const { return *this; }
};
int f [[deprecated("old")]] (int);
template<typename T> T tf [[nodiscard]] (T t) { return t; }
void u(S s) {
  s(1);
  s.g(2);
  s[3];
  s.g<long>(5);
  fr(s);
  s + s;
  s.operator()(4);
  f(1);
  tf(1);
}
