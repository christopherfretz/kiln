// [intro.compliance.general]/7 [dcl.attr.grammar]/5 Under
// `-ffreestanding` a `memset` written as a byte loop is not turned back
// into a call of `memset` at -O2 (every function is `"no-builtins"`, so
// LLVM's loop-idiom recognition leaves it), which would recurse forever.
// spec/codegen.md "no-builtin", WP-808.
//@ expect: run
//@ strict-diags
//@ args: -O2 -ffreestanding
extern "C" void* memset(void* d, int c, unsigned long n) {
  unsigned char* p = static_cast<unsigned char*>(d);
  for (unsigned long i = 0; i < n; ++i)
    p[i] = static_cast<unsigned char>(c);
  return d;
}
void* (*volatile fill)(void*, int, unsigned long) = memset;
// A freestanding `main` is an ordinary (mangled) function.
extern "C" int main() {
  static_assert(__STDC_HOSTED__ == 0);
  unsigned char buf[300];
  fill(buf, 7, sizeof buf);
  int sum = 0;
  for (unsigned char b : buf)
    sum += b;
  return sum == 2100 ? 0 : 1;
}
