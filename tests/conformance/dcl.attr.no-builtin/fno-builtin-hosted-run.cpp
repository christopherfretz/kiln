// [dcl.attr.grammar]/5 A `no_builtin("memset")` function defining a byte
// loop is not turned into a call of `memset` at -O2 either (its
// `"no-builtin-memset"`): the attribute is how a hosted program writes
// its own `memset`-like routine. spec/codegen.md "no-builtin", WP-808.
//@ expect: run
//@ strict-diags
//@ args: -O2
extern "C" __attribute__((no_builtin("memset"))) void* memset(void* d, int c, unsigned long n) {
  unsigned char* p = static_cast<unsigned char*>(d);
  for (unsigned long i = 0; i < n; ++i)
    p[i] = static_cast<unsigned char>(c);
  return d;
}
int main() {
  unsigned char buf[300];
  memset(buf, 7, sizeof buf);
  int sum = 0;
  for (unsigned char b : buf)
    sum += b;
  return sum == 2100 ? 0 : 1;
}
