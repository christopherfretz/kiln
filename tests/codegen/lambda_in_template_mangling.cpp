// Lambdas in templated contexts get a numbered `Ul...E_` closure name
// (linkonce_odr), not an internal `$_N` (clang getCurrentMangleNumberContext:
// numbered in a template instantiation or a dependent context): generic and
// non-generic lambdas in a function template and in an out-of-line member of
// a class template, a static local's lambda, and a variable template's
// lambda, whose closure prefix carries the template arguments (`2vtIiEM`).
template<class T> int ft(T t) {
  auto a = [](auto x) { return x; };
  auto c = [](int x) { return x + 1; };
  return a(t) + c(2);
}
template<class T> struct C { int m(T t); static int s(); };
template<class T> int C<T>::m(T t) {
  auto a = [](auto x) { return x; };
  auto b = [](auto x) { return x * 2; };
  auto d = [t](auto x) { auto e = [x] { return x; }; return e() + t; };
  return a(t) + b(t) + d(2);
}
template<class T> int C<T>::s() { static auto l = [] { return 5; }; return l(); }
template<class T> constexpr auto vt = [](T x) { return x; };
int main() {
  C<int> ci; C<long> cl;
  return ft(1) + ft(2L) + ci.m(1) + cl.m(2) + C<int>::s() + vt<int>(1) + vt<long>(2);
}
