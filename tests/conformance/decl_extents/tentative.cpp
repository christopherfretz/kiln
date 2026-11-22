// The declaration-extent table (spec/ast.md §19.6): the parser's
// disambiguation scans tokens without parsing them, so a statement or
// declaration it looks at first is recorded once, by the parse that is
// kept; late-parsed member bodies, default arguments and member
// initializers keep the ranges of their captured tokens, and a lambda
// in a template head's default argument does not take the template's
// first token.
//@ decl-extents: tentative.extents
//@ std: c++20
//@ args: -Wno-vexing-parse
struct T {
  T();
  T(int);
  int v = T::k + 1;
  static const int k = 2;
  int get(int d = k) const { return v + d; }
};
int x = 0;
void stmts() {
  T(y);
  T(z) = T(1);
  T(*p)(int);
  int(x) = 3;
  T t(T(int));
}
template <class U = decltype([] {
  int local = 1;
  return local;
}())>
struct L {};
