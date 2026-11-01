//@ expect: run
// [temp.deduct.call]/1: a trailing function parameter pack deduces its
// own elements from the remaining arguments even when an earlier
// template parameter pack was deduced from a type (`tys<Tf...>`); the
// explicitly specified prefix is that of the trailing pack alone.
template <class... T> struct tys {};
template <class... Tf, class... Up> int f(tys<Tf...>, Up... u) {
  return int(sizeof...(Tf)) * 10 + int(sizeof...(Up));
}
template <class X, class... Up> int g(tys<X>, Up...) { return sizeof...(Up); }
template <class... Tf, class... Up> int h(tys<Tf...>, Up... u) { return (0 + ... + u); }
int main() {
  if (f(tys<int>(), 1, 2) != 12) return 1;
  if (f(tys<int, char, long>(), 1) != 31) return 2;
  if (f(tys<>()) != 0) return 3;
  if (g(tys<int>(), 1, 2, 3) != 3) return 4;
  if (h(tys<char>(), 4, 5) != 9) return 5;
  if (f<int>(tys<int>(), 'a') != 11) return 6;
  return 0;
}
