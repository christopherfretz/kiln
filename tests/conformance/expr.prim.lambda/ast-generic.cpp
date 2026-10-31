//@ expect: ast
//@ ast-filter: gfun
// [expr.prim.lambda.closure]/3-4: a generic lambda's call operator is a
// function template; each `auto` parameter invents a template type
// parameter (`x:auto`) after any explicit ones, and constraints become
// the template's associated constraints.
template <class T> concept Small = sizeof(T) <= 4;
void gfun() {
  auto id = [](auto x) { return x; };
  auto tp = []<class T>(T a, T b) { return a < b ? b : a; };
  auto cn = [](Small auto x) { return x + 1; };
  auto da = [](auto& x) -> decltype(auto) { return (x); };
  auto va = [](auto... xs) { return (0 + ... + xs); };
  auto rq = []<class T> requires Small<T> (T x) { return x; };
  int k = 1;
  auto cp = [k](auto x) { return x + k; };
}
