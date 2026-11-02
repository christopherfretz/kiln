// [temp.deduct.general]/8, [temp.alias]/2, [stmt.ranged]/1: the implicit
// `operator!=`, `operator*` and `operator++` calls of a range-based `for`
// resolve overloads; a candidate whose substitution fails through an alias
// template (in a default template argument, in the return type, or in a
// `void_t` partial specialization matched during deduction) is silently
// dropped and the other candidate is used (libc++ `__enable_if_t`).
//@ expect: compile-ok
//@ strict-diags
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template <class...> using void_t = void;
template <class T> struct is_ptr { static constexpr bool value = false; };
template <class T> struct is_ptr<T*> { static constexpr bool value = true; };
template <class T, class = void> struct has_ptr { static constexpr bool value = false; };
template <class T> struct has_ptr<T, void_t<typename T::pointer>> { static constexpr bool value = true; };
template <class T> using id_t = T;
namespace n {
template <class P> struct It {
  int& operator*() const;
  template <class Q = P, enable_if_t<is_ptr<Q>::value, int> = 0> int& operator*() const;
  It& operator++();
  template <class Q = P, class = enable_if_t<has_ptr<Q>::value>> It& operator++();
};
template <class P> bool operator!=(It<P>, It<P>);
template <class P, enable_if_t<is_ptr<P>::value, int> = 0> bool operator!=(It<P>, It<P>);
template <class P> id_t<enable_if_t<has_ptr<P>::value, bool>> operator!=(It<P>, It<P>);
}
struct V { n::It<int> begin(); n::It<int> end(); };
void test(V& v) { for (int& x : v) (void)x; }
template <class T> void g(T& v) { for (auto& x : v) (void)x; }
template void g(V&);
