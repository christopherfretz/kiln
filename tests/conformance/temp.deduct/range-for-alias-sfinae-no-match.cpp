// [temp.deduct.general]/8, [stmt.ranged]/1, [over.match.viable]: when every
// candidate of a range-based `for`'s implicit `operator!=` fails substitution
// through an alias template, the error is the "invalid operands" one, with
// each candidate's deduction-failure note (clang 19's `enable_if_t` form,
// spec/sema-templates.md §5.7.1) and the range-for notes after them.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:48 error "invalid operands to binary expression ('n::It<int>' and 'n::It<int>')"
//@ diag: 33:15 note "in instantiation of function template specialization 'f<V>' requested here"
//@ diag: 28:65 note "candidate template ignored: requirement 'is_ptr<int>::value' was not satisfied [with P = int]"
//@ diag: 29:62 note "candidate template ignored: requirement 'is_ptr<int>::value' was not satisfied [with P = int]"
//@ diag: 32:50 note "in implicit call to 'operator!=' for iterator of type 'V'"
//@ diag: 31:23 note "selected 'begin' function with iterator type 'n::It<int>'"
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template <class T> struct is_ptr { static constexpr bool value = false; };
template <class T> struct is_ptr<T*> { static constexpr bool value = true; };
template <class T> using id_t = T;
namespace n {
template <class P> struct It {
  P p;
  int& operator*() const;
  template <class Q = P, enable_if_t<is_ptr<Q>::value, int> = 0> int& operator*() const;
  It& operator++();
  template <class Q = P, class = enable_if_t<is_ptr<Q>::value>> It& operator++();
};
template <class P, enable_if_t<is_ptr<P>::value, int> = 0> bool operator!=(It<P>, It<P>);
template <class P> id_t<enable_if_t<is_ptr<P>::value, bool>> operator!=(It<P>, It<P>);
}
struct V { n::It<int> begin(); n::It<int> end(); };
template <class T> void f(T& v) { for (auto& x : v) {} }
template void f(V&);
