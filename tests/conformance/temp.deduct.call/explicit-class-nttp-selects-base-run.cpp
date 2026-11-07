// [temp.deduct.call]/4.3, [temp.param]/8: an explicitly given class-type
// template argument substituted into `P` is a value, not a deduced
// parameter; matched against each base of `A` it selects only the base
// whose argument is the same template parameter object. A base whose
// argument is an integer or another class value is no match, so the
// call is not ambiguous.
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 0 3
extern "C" int printf(const char *, ...);
template <class C, unsigned long N> struct lit {
  C s[N];
  constexpr lit(const C (&a)[N]) {
    for (unsigned long i = 0; i < N; ++i)
      s[i] = a[i];
  }
};
struct L { int v; };
template <auto Tag, unsigned long A> struct entry {
  static constexpr unsigned long idx = A;
};
template <class... E> struct map : E... {};
template <auto Tag, unsigned long A> entry<Tag, A> get(entry<Tag, A>);
template <class T> T &&dv();
using M = map<entry<lit<char, 3>{"s1"}, 0>, entry<lit<char, 3>{"s2"}, 1>,
              entry<5, 2>, entry<L{1}, 3>>;
int main() {
  constexpr auto t = lit<char, 3>{"s2"};
  using E = decltype(get<t>(dv<M>()));
  using E2 = decltype(get<5>(dv<M>()));
  using E3 = decltype(get<lit<char, 3>{"s1"}>(dv<M>()));
  using E4 = decltype(get<L{1}>(dv<M>()));
  printf("%lu %lu %lu %lu\n", E::idx, E2::idx, E3::idx, E4::idx);
}
