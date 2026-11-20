// [temp.arg.nontype]/2-3 with GNU complex types: the real and imaginary
// parts of a complex object (`__real__ z`, `__imag__ z`, lvalues of the
// element type) are subobjects a reference or pointer template argument
// may designate, as clang 19 and 22 accept. Substituting one names the
// part again (WP-1743; kiln used to stop with "no expression stands for
// the non-type template argument").
//@ expect: compile-ok
//@ strict-diags
struct C { _Complex double z; };
constexpr C gc{1.0 + 2.0i};
constexpr _Complex float cf = 3.0f + 4.0if;
template <const double& R> struct X { static constexpr double v = R; };
template <const float* P> struct Y { static constexpr float v = *P; };
static_assert(X<__real__ gc.z>::v == 1.0);
static_assert(X<__imag__ gc.z>::v == 2.0);
static_assert(Y<&__imag__ cf>::v == 4.0f);
static_assert(Y<&__real__ cf>::v == 3.0f);
