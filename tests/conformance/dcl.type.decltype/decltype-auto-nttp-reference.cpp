// [temp.param]/6, [dcl.type.decltype]/1.2: a decltype(auto) parameter
// deduced from an lvalue of type 'const X&' has that reference type. From
// clang's CXX/dcl.dcl/dcl.spec/dcl.type/dcl.type.decltype/p1.cpp. Found by
// the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
template <decltype(auto) v> constexpr bool is_const_ref = same<decltype(v), const struct X &>;
struct X {};
template <auto x> auto &TempParamObject = x;
static_assert(is_const_ref<TempParamObject<X{}>>);
