// [dcl.type.decltype]/1.3, [temp.param]/8: decltype of a non-type template
// parameter of reference type is the reference type, also when the
// referenced type is a class (written `const X &`, `auto &`, or
// `decltype(auto)` deduced as a reference) and when the referent is a
// template parameter object; a parameter of class type names its template
// parameter object, whose decltype is the unqualified class (WP-958).
//@ std: c++20
//@ expect: compile-ok
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
struct X {};
const X gx{};
const X &rx = gx;
template <auto x> auto &param_object = x;

template <const X &v> constexpr bool b1 = same<decltype(v), const X &>;
static_assert(b1<gx>);
static_assert(b1<param_object<X{}>>);
template <auto &v> constexpr bool b2 = same<decltype(v), const X &>;
static_assert(b2<gx>);
template <decltype(auto) v> constexpr bool b3 = same<decltype(v), const X &>;
static_assert(b3<rx>);
static_assert(b3<(gx)>);
static_assert(b3<param_object<X{}>>);
template <X v> constexpr bool b4 = same<decltype(v), X>;
static_assert(b4<X{}>);
template <decltype(auto) v> constexpr bool b5 = same<decltype(v), X>;
static_assert(b5<X{}>);
template <decltype(auto) v> struct W { using type = decltype(v); };
static_assert(same<W<rx>::type, const X &>);
static_assert(same<W<X{}>::type, X>);
