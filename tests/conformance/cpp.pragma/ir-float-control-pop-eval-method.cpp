// clang `-ffp-eval-method=double` with `#pragma float_control(push)` and
// `pop` at file and namespace scope: the pop applies the restored state to
// `FPOptions(LangOpts)`, which does not hold the command line's method
// (the preprocessor does), so from there on operations evaluate in their
// own type, even though the restored state is empty; `__FLT_EVAL_METHOD__`
// stays 1, and an instantiation is `source` as before (clang 19 and 22).
// Compared with clang's IR exactly (spec/driver.md "Floating-point flags",
// WP-1808).
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -std=c++23 -ffp-eval-method=double
float f0(float a, float b) { return a * b; }
constexpr float k0 = (16777216.0f + 1.0f) - 16777216.0f;
float g0 = k0;
#pragma float_control(push)
#pragma float_control(pop)
int m1 = __FLT_EVAL_METHOD__;
float f1(float a, float b) { return a * b; }
constexpr float k1 = (16777216.0f + 1.0f) - 16777216.0f;
float g1 = k1;
template <class T> T t1(T a, T b) { return a * b; }
float u1(float a) { return t1(a, a); }
namespace n {
#pragma float_control(push)
#pragma float_control(pop)
float f2(float a, float b) { return a * b; }
}
