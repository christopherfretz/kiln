//@ expect: run
//@ requires: target-x86_64
//@ requires: clang>=22
//@ oracle: none
//@ stdout: 3f80 4000 4040 c0a0 7f80 3f80
// [basic.extended.fp]/[expr.call]: a `__bf16` call result passed straight
// on as an argument (spec/abi.md §8.18). LLVM 19's x86-64 FastISel (-O0)
// takes such a result for the already widened `float` and truncates its raw
// bits; kiln spills the result through memory when its backend is LLVM 19
// and it does not optimize. clang 19 miscompiles this program (hence
// `clang>=22` and no oracle); clang 22 and kiln print the values below.
// The constant is a cast, not `1.0bf16`: clang 22 rejects the `bf16`
// literal suffix on x86-64.
extern "C" int printf(const char *, ...);

__attribute__((noinline)) __bf16 one() { return static_cast<__bf16>(1.0f); }
__attribute__((noinline)) __bf16 twice(__bf16 x) { return x + x; }
__attribute__((noinline)) __bf16 add(__bf16 x, __bf16 y) { return x + y; }
__attribute__((noinline)) __bf16 neg(__bf16 x) { return -x; }
__attribute__((noinline)) __bf16 big() { return static_cast<__bf16>(__builtin_inff()); }

unsigned short bits(__bf16 x) {
  unsigned short r;
  __builtin_memcpy(&r, &x, sizeof r);
  return r;
}

int main() {
  printf("%04x %04x %04x %04x %04x %04x\n", bits(one()), bits(twice(one())),
         bits(add(one(), twice(one()))), bits(neg(add(twice(one()), add(one(), twice(one()))))),
         bits(twice(big())), bits([] { return one(); }()));
}
