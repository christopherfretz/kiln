// GNU `_Complex __bf16` (GCC 14; libstdc++ <complex> spells it
// `__complex__ decltype(0.0bf16)` under __STDCPP_BFLOAT16_T__, which kiln
// predefines as GCC does): a pair of __bf16 (size 4, align 2), arithmetic
// computed in `_Complex float` and rounded once (GCC 14 calls __mulsc3 /
// __divsc3, as clang does for `_Complex _Float16` on x86-64), constant
// evaluation, and the call ABI (an HFA of two __bf16 on aarch64, one SSE
// register on x86-64). Expected output from g++-14 on aarch64.
//@ expect: run
//@ strict-diags
//@ skip(clang): clang 19/22 reject `_Complex __bf16` ("'_Complex type-name' is invalid")
//@ stdout: 5 2.5 3.5 1 1.5 2 5 1 -1
#include <cstdio>
typedef __complex__ decltype(0.0bf16) CB;
static_assert(sizeof(CB) == 4 && alignof(CB) == 2);
static_assert(__is_same(decltype(__real__ CB()), __bf16));
constexpr CB mk(__bf16 r, __bf16 i) {
  CB z = {r, i};
  return z;
}
constexpr CB m = mk(1.5bf16, 2.0bf16) * mk(2.0bf16, -1.0bf16);
static_assert(__real__ m == 5.0bf16 && __imag__ m == 2.5bf16);
constexpr CB s = mk(1.5bf16, 2.0bf16) + mk(2.0bf16, -1.0bf16);
static_assert(__real__ s == 3.5bf16 && __imag__ s == 1.0bf16);
static_assert(mk(1, 2) == mk(1, 2) && mk(1, 2) != mk(2, 1));
CB mul(CB a, CB b) { return a * b; }
CB add(CB a, CB b) { return a + b; }
CB dv(CB a, CB b) { return a / b; }
CB sub_real(CB a, __bf16 b) { return a - b; }
void mul_assign(CB &a, CB b) { a *= b; }
int main() {
  CB a = mk(1.5bf16, 2.0bf16), b = mk(2.0bf16, -1.0bf16);
  CB c = mul(a, b), d = add(a, b), e = dv(c, b);
  _Complex float f = c;
  CB g = sub_real(b, 3.0bf16);
  mul_assign(g, mk(0, 1));
  std::printf("%g %g %g %g %g %g %g %g %g\n", (float)__real__ c, (float)__imag__ c,
              (float)__real__ d, (float)__imag__ d, (float)__real__ e, (float)__imag__ e,
              __real__ f, (float)__real__ g, (float)__imag__ g);
}
