// [expr.prim.req.type], [expr.prim.req.simple]: `typename T::type()` and
// `typename T::type{}` in a requirement-body are simple requirements (an
// explicit type conversion), not type requirements. Found running GCC's
// C++ testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++20
template <class T, class U> concept Same = __is_same(T, U);
struct A { using type = int; };
struct B { struct type { int v = 3; }; };
struct N {};
template <class T> concept Paren = requires { typename T::type(); };
template <class T> concept Brace = requires { typename T::type{}; };
template <class T> concept Both = requires { typename T::type; typename T::type(); };
template <class T>
concept Compound = requires { { typename T::type() } -> Same<typename T::type>; };
static_assert(Paren<A> && Paren<B> && !Paren<N>);
static_assert(Brace<A> && Brace<B> && !Brace<N>);
static_assert(Both<A> && !Both<N>);
static_assert(Compound<A> && !Compound<N>);
template <class T> struct W { using type = T; };
auto f = [](auto x) requires requires { typename W<decltype(x)>::type(0); } { return 1; };
int g() { return f(0); }
