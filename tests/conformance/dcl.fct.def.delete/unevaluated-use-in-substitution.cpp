// [dcl.fct.def.delete]/2, [temp.deduct.general]/8: naming a deleted
// function is ill-formed only when overload resolution selects it, and in
// a substitution it is then a substitution failure, not a hard error: a
// deleted explicit specialization of a member template named in a partial
// specialization's argument or in a function template's decltype makes
// that one not match, and taking the address of a deleted member in the
// return type removes the candidate. A member named without an object in
// an unevaluated operand of a substitution is a failure too. An undefined
// function template named in an unevaluated operand is not instantiated,
// and an explicit instantiation declaration of an undefined member needs
// no definition ([temp.explicit]/12). Found running GCC's C++ testsuite.
//@ strict-diags
struct X { template<int> static int g(); static void h() = delete; };
template<> int X::g<0>() = delete;
struct Y { template<int> static int g(); static void h(); };
template<class T, class = void> struct C0 { static const bool value = false; };
template<class T> struct C0<T, decltype(void(T::template g<0>()))> { static const bool value = true; };
static_assert(!C0<X>::value, "");
static_assert(C0<Y>::value, "");
template<class T, class = void> struct C1 { static const bool value = false; };
template<class T> struct C1<T, decltype(void(T::template g<0>))> { static const bool value = true; };
static_assert(!C1<X>::value, "");
static_assert(C1<Y>::value, "");
template<class T, class = void> struct C2 { static const bool value = false; };
template<class T> struct C2<T, decltype(void(T::h))> { static const bool value = true; };
static_assert(!C2<X>::value, "");
static_assert(C2<Y>::value, "");
template<class T> auto f(int) -> decltype(T::template g<0>(), 1) { return 1; }
template<class T> long f(...) { return 2; }
static_assert(sizeof(f<X>(0)) == sizeof(long), "");
static_assert(sizeof(f<Y>(0)) == sizeof(int), "");
template<class T> auto p(int) -> decltype(&T::h, 1);
template<class T> long p(...);
static_assert(sizeof(p<X>(0)) == sizeof(long), "");
static_assert(sizeof(p<Y>(0)) == sizeof(int), "");
// Not instantiated by an unevaluated use.
template<class T> T undef(T);
decltype(undef(1)) u = 0;
static_assert(sizeof(undef(1.0)) == sizeof(double), "");
// An explicit instantiation declaration of undefined members.
template<class T> struct S { void m(); static int s; };
extern template struct S<int>;
extern template void S<long>::m();
struct bar { int x(); };
template<class C> constexpr int st(decltype(void(C::x), 0)) { return 1; }
template<class C> constexpr int st(...) { return 0; }
static_assert(st<bar>(0) == 0, "");
template<class C> constexpr int su(decltype(sizeof(C::x))) { return 1; }
template<class C> constexpr int su(...) { return 0; }
static_assert(su<bar>(0) == 0, "");
