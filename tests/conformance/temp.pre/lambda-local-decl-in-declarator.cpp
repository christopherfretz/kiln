// [temp.pre]/2, [expr.prim.lambda.closure]: a template-head heads the
// declaration whose declarator follows it. A lambda-expression in that
// declarator (a trailing return type, a default argument, a
// decl-specifier) has a body whose block-scope declarations are not
// headed by it: `h` stays a function template, found as one by
// `h<int>` and `::h<int>`.
//@ expect: compile-ok
template <class T> auto h() -> decltype([] { T t{}; return t; }());
template <class T> auto h2(T) -> decltype([] { int t = 0; return t; }());
template <class T> T h3(T, int = [] { int t{}; struct L { int m; }; return t + L{}.m; }());
template <class T> decltype([] { T u{}; return u; }()) h4();
static_assert(__is_same(decltype(h<int>()), int));
static_assert(__is_same(decltype(::h<long>()), long));
static_assert(__is_same(decltype(h2<int>(1)), int));
static_assert(__is_same(decltype(h2(1)), int));
static_assert(__is_same(decltype(&h2<char>), int (*)(char)));
static_assert(__is_same(decltype(h3<short>(1)), short));
static_assert(__is_same(decltype(h4<char>()), char));
