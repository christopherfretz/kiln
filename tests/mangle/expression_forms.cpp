// Mangling differential: expression forms in a dependent `decltype`
// ([temp.over.link]/5; spec/abi.md §8.30, WP-980), each as clang 19/22
// `mangleExpression` encodes it:
// - `delete` `dl`, `delete[]` `da`, `::delete` `gs dl`;
// - `__real__`/`__imag__` as their operand alone;
// - an integer literal past the signed 64-bit range in decimal;
// - a compound assignment `pL`, `aN`, ... as a binary operator.
template <class T> auto del(T t) -> decltype(delete t);
template <class T> auto dela(T t) -> decltype(delete[] t);
template <class T> auto gdel(T t) -> decltype(::delete t);
template <class T> auto real(T t) -> decltype(__real__ t);
template <class T> auto imag(T t) -> decltype(__imag__ t);
template <class T> auto wide(T t) -> decltype(t + 18446744073709551615ull);
template <class T> auto cadd(T t) -> decltype(t += 1);
template <class T> auto cand(T t) -> decltype(t &= 1, t <<= 2);

void use() {
  int *p = nullptr;
  _Complex double c = 0;
  del(p);
  dela(p);
  gdel(p);
  real(c);
  imag(c);
  wide(1);
  int i = 0;
  cadd(i);
  cand(i);
}
