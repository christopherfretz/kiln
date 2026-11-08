// Mangled names and IR of specializations whose template argument is a
// GNU `_Complex` value (`tl <type> <parts> E`, trailing `+0` parts
// dropped, as clang's `mangleValueInTemplateArg`); conformance in
// tests/conformance/temp.arg.nontype/complex-argument-run.cpp. WP-810.
typedef _Complex float cf;
typedef _Complex double cd;
typedef _Complex int ci;
template <cf C> struct A { static cf get() { return C; } };
template <cd D> double f() { return __real__ D + __imag__ D; }
template <ci I> int g() { return __imag__ I; }
template <auto X> int h() { return sizeof(X); }
double run() {
  cf a = A<cf{1.0f, 2.0f}>::get();
  return __real__ a + f<cd{3.0, 4.0}>() + g<ci{1, 2}>() + h<cf{1, 0}>() + h<cf{0, 0}>() +
         h<cf{0, 1}>() + h<cf{-0.0f, 0}>();
}
