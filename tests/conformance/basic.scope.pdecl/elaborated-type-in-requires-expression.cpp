// [basic.scope.pdecl]/7.2, [dcl.type.elab]/4: an elaborated-type-specifier
// `struct Y` that declares a new class inside a requires-expression (here a
// template argument of a member call) declares it in the nearest enclosing
// namespace, not in the requires-expression's parameter scope; the
// requirement holds or fails by the member as for a class named by any
// other means. Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-ok
struct X;
template <class T>
concept C1 = requires(T& t) { t.template f<X>(); };
template <class T>
concept C2 = requires(T& t) { t.template f<struct Y>(); };

struct good { template <class T> void f() {} };
struct bad {};
static_assert(C1<good>);
static_assert(C2<good>);
static_assert(!C1<bad>);
static_assert(!C2<bad>);

// Y is a namespace-scope class.
struct Y { int y; };
static_assert(sizeof(::Y) == sizeof(int));

template <class T> int g(T& t) { t.template f<struct Z>(); return 0; }
int h(good x) { return g(x); }
struct Z {};
