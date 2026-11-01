// [stmt.ranged]/1, [temp.deduct.general]/8: overload resolution for
// `__begin != __end` considers the rewritten `operator==` candidates
// ([over.match.oper]/3.4); deducing a converting constructor template for
// one of them fails in its default template argument, a substitution
// failure, not a hard error, inside the range-for step (libc++
// `error_condition` with `__wrap_iter`).
//@ expect: compile-ok
//@ strict-diags
namespace s {
template <bool, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template <class T> struct is_ec { static const bool value = false; };
struct ec {
  template <class E, enable_if_t<is_ec<E>::value, int> = 0> ec(E);
};
bool operator==(const ec&, const ec&);
template <class I> struct it {
  I p;
  int& operator*() const;
  it& operator++();
};
template <class A, class B> bool operator==(const it<A>&, const it<B>&);
template <class A> bool operator!=(const it<A>&, const it<A>&);
struct vec {
  it<int*> begin() const;
  it<int*> end() const;
};
}  // namespace s

int f(const s::vec& v) {
  int n = 0;
  for (int e : v) n += e;
  return n;
}
