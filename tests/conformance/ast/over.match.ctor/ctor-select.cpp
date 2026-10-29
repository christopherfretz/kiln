//@ expect: ast
//@ oracle: clang
// [over.match.ctor], [over.match.copy]: the selected constructor of a
// direct-, copy- and list-initialization, and a user-defined conversion
// through a converting constructor in a call argument.
struct S {
  S(int);
  S(double, int = 0);
  explicit S(const char *);
};
void take(S);
void t() {
  S a(1);
  S b = 2.5;
  S c{3, 4};
  S d("x");
  take(5);
}
