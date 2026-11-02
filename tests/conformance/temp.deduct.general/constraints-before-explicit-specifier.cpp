// [temp.deduct.general]/5, [temp.deduct.general]/7, [dcl.fct.spec]/3:
// the associated constraints of a deduced constructor template are
// checked before its explicit-specifier is substituted, so a candidate
// whose constraint fails never instantiates an `explicit(expr)` that
// would be ill-formed for its arguments (clang
// `instantiateExplicitSpecifierDeferred`). A satisfied constraint still
// substitutes it: `explicit(true)` excludes the constructor from
// copy-initialization ([over.match.copy]/1.1).
// libstdc++ 14 <tuple> `tuple(_UTuple&&)` with
// `__eligible_tuple_like<tuple> _UTuple` and
// `explicit(!__convertible_from_tuple_like<_UTuple>())`, reached by
// `std::tuple<int> t(1)` under C++23 (bits/stl_pair.h has the same
// shape for pair-like arguments; WP-366).
// Verified against clang 19.
//@ expect: run
template<class T> struct is_tl { static constexpr bool value = false; };
template<class T> struct is_tl<T*> { static constexpr bool value = true; };
template<class T> concept tl = is_tl<T>::value;

struct ptr_only {};

template<class T> struct box {
  // Ill-formed for any U without a member `ok`.
  template<class U> static consteval bool conv() { return U::ok; }
  template<tl U> explicit(!conv<U>()) box(U) : v(2) {}
  template<class U> requires tl<U> && (sizeof(U) > 1)
  explicit(U::nope) box(U, int) : v(3) {}
  box(int) : v(1) {}
  box(long, int) : v(4) {}
  int v;
};

struct ok_true { static constexpr bool ok = true; };
template<> struct is_tl<ok_true> { static constexpr bool value = true; };
struct ok_false { static constexpr bool ok = false; };
template<> struct is_tl<ok_false> { static constexpr bool value = true; };

int main() {
  box<int> a(1);          // tl<int> fails: conv<int>() is never formed
  box<int> b(1L, 0);      // tl<long> fails: U::nope is never formed
  box<int> c = ok_true{}; // explicit(false): a converting constructor
  box<int> d(ok_false{}); // explicit(true): direct-initialization only
  return a.v == 1 && b.v == 4 && c.v == 2 && d.v == 2 ? 0 : 1;
}
