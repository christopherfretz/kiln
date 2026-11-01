// [temp.deduct.call]/1, CWG1391: a function parameter whose type contains
// no template parameter that participates in deduction (here the pack `A`
// appears only in non-deduced contexts, behind an alias of a member
// typedef) takes no part in deduction, so its argument is not matched
// against it there; it is checked by overload resolution after
// substitution (libc++ `std::print(format_string<_Args...>, _Args&&...)`).
// Verified against clang 19.
//@ expect: run
template <class T> struct ti { using type = T; };
template <class T> using ti_t = typename ti<T>::type;
template <class C, class... A> struct BFS { consteval BFS(const C*) {} };
template <class... A> using FS = BFS<char, ti_t<A>...>;
template <class... A> int p(int, FS<A...>, A&&...) { return sizeof...(A); }
template <class... A> int q(int, BFS<char, ti_t<A>...>, A&&...) { return 10; }
template <class A> int r(int, BFS<char, ti_t<A>>, A&&) { return 100; }
int main() {
  int n = p(1, "x") + p(1, "x", 2) + q(1, "x") + r(1, "x", 2);
  return n == 111 ? 0 : 1;
}
