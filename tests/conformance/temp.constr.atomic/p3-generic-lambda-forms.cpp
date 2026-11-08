// [temp.constr.atomic]/3, [expr.prim.lambda.general]/6: generic lambdas in
// constraint expressions get their own template head, one level deeper
// than the enclosing template's, whether the lambda has several invented
// parameters or a pack, an explicit template parameter list next to
// 'auto', its own trailing requires-clause inside a requires-expression,
// sits in a requires-clause, in a variable template's initializer, or
// nests another generic lambda. Verified against clang 19 and GCC 16.1.
//@ expect: compile-ok
//@ oracle: gcc
template <class T> concept K = [](auto x) { return sizeof(x) > 1; }(T{});
template <class T> concept K2 = [](auto x, auto... ys) { return sizeof(x) + sizeof...(ys) > 1; }(T{}, 1);
template <class T> concept K3 = []<class U>(U, auto y) { return sizeof(U) == sizeof(y); }(T{}, T{});
template <class T> concept K4 = requires { [](auto x) requires (sizeof(x) > 1) {}(T{}); };
template <class T> requires ([](auto x) { return sizeof(x) == 4; }(T{})) constexpr int g(T) { return 4; }
template <class T> constexpr int g(T) { return 0; }
template <class T> constexpr auto v = [](auto x) { return sizeof(x) + sizeof(T); }(T{});
static_assert(K<long> && !K<char>);
static_assert(K2<char>);
static_assert(K3<int>);
static_assert(K4<long> && !K4<char>);
static_assert(g(1) == 4 && g('a') == 0);
static_assert(v<int> == 8);
template <class T> concept N = [](auto x) { return [](auto y) { return sizeof(y); }(x) > 1; }(T{});
static_assert(N<long> && !N<char>);
