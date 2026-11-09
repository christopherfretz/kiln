// Mangling differential: GNU imaginary literals in dependent signatures
// ([temp.over.link]/5; spec/abi.md "Mangling", WP-1029). clang mangles
// `ImaginaryLiteral` as a complex literal (Itanium `<expr-primary>`):
// `L <complex type> <real> _ <imag> E`, the real part a zero of the
// element type (`0` for an integer, the element's width of hex zeros for
// a floating type) and the imaginary part the literal's decimal value or
// hex encoding. `long double` is 128-bit IEEE on AArch64 and the 80-bit
// x87 format on x86-64, so `iml`/`a6` differ per triple.
template <int N> struct S {};
template <class T> auto im(T t) -> decltype(t + 2.0i);
template <class T> auto imf(T t) -> decltype(t + 2.0fi);
template <class T> auto iml(T t) -> decltype(t + 2.0li);
template <class T> auto imi(T t) -> decltype(t + 2i);
template <class T> auto a1(T t) -> decltype(t + 2ui);
template <class T> auto a2(T t) -> decltype(t + 2li);
template <class T> auto a3(T t) -> decltype(t + 3ulli);
template <class T> auto a4(T t) -> decltype(t + 0.5i);
template <class T> auto a5(T t) -> decltype(t * 1.5fi);
template <class T> auto a6(T t) -> decltype(t - 0.25li);
template <class T> auto a8(T t) -> decltype(t + 0x10i);
template <class T> auto a10(T t) -> S<sizeof(t + 1i)>;
template <class T> auto bc1(T t) -> decltype(t + __builtin_complex(1.0, 2.0));
template <class T> auto bc2(T t) -> decltype(__builtin_complex(t, t));

void use() {
  im(1.0);
  imf(1.0f);
  iml(1.0);
  imi(1);
  a1(1);
  a2(1);
  a3(1);
  a4(1.0);
  a5(1.0f);
  a6(1.0);
  a8(1);
  a10(1);
  bc1(1.0);
  bc2(1.0);
}
