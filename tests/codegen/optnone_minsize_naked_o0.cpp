//@ args: -O0
// GNU `optnone` / `minsize` / `naked` on a definition at -O0 (clang
// `SetLLVMFunctionAttributesForDefinition`, WP-405): `optnone` gives
// `noinline optnone` at every level and overrides `always_inline` and
// `minsize`; -O0 implies `optnone` except with `minsize` (`minsize
// noinline`) or `always_inline`; `naked` adds `naked` (and `noinline`)
// and the body has no parameter prologue and ends in `unreachable`;
// without `optnone`, `minsize` adds `minsize` and `cold` adds `optsize`
// unless -O0 implied `optnone`. Declarations and call sites get none of
// them. Falling off a value-returning function is `llvm.trap` +
// `unreachable` at -O0, `unreachable` alone above it.
__attribute__((optnone)) int on() { return 1; }
__attribute__((minsize)) int ms(int x) { return x * 3; }
__attribute__((naked)) void nk() { asm("nop"); }
__attribute__((naked)) int nk_ret(int x) { asm("nop"); }
__attribute__((naked)) void nk_empty() {}
__attribute__((minsize, always_inline)) inline int ms_ai(int x) { return x; }
__attribute__((minsize, noinline)) int ms_ni(int x) { return x; }
__attribute__((optnone, cold)) int on_cold() { return 2; }
__attribute__((minsize, cold)) int ms_cold(int x) { return x + 1; }
__attribute__((cold)) int cold(int x) { return x + 2; }
__attribute__((minsize)) int ms_decl(int x);
__attribute__((optnone)) int on_decl(int x);
__attribute__((naked)) int nk_decl(int x);
struct S {
  __attribute__((naked)) int m(int a) { asm("nop"); }
};
int fall_off() {}
int use(S s, int x) {
  return ms_ai(x) + ms_decl(x) + on_decl(x) + nk_decl(x) + ms_cold(x) + s.m(x);
}
