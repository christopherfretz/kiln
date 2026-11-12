// [bit.cast] (the working draft, P4101R1): `std::meta::info` is scalar,
// hence trivially copyable ([basic.types.general]/9), so a bit cast from
// or to it, or a class holding one, is well-formed; only a constant
// evaluation of one is not a constant expression (bit-cast-info-neg.cpp).
// At run time an `info` is a pointer-sized null (spec/codegen.md
// "Run-time reflections"), so the round trip of the null reflection gives
// zero bits and back. kiln only: GCC 16.1 rejects each cast ("cannot be
// used with consteval-only types"; docs/status/deviations.md).
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ stdout: 0 1 0 1 1 1
extern "C" int printf(const char*, ...);
using info = decltype(^^int);
struct S { info r; };
struct L { long r; };
static_assert(__is_trivially_copyable(info) && __is_trivially_copyable(S));
template <class T> long bits(T t) { return __builtin_bit_cast(long, t); }
int main() {
  info z{};
  long n = __builtin_bit_cast(long, z);
  info back = __builtin_bit_cast(info, n);
  L l = __builtin_bit_cast(L, S{});
  S s = __builtin_bit_cast(S, L{0});
  printf("%ld %d %ld %d %d %d\n", n, back == info{}, l.r, s.r == z,
         bits(z) == 0, __builtin_bit_cast(info, 0L) == z);
}
