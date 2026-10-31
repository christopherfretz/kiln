//@ expect: ast
//@ ast-implicit
//@ ast-filter: gfun
// [expr.prim.lambda.closure]/9: a captureless generic lambda's
// conversion function and static invoker are templates over the call
// operator's template parameters (`operator auto (*)(type-parameter-0-0)`,
// `__invoke`); a capturing one has neither.
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
