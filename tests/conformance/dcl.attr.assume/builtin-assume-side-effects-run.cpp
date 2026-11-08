// [dcl.attr.assume] A `__builtin_assume` argument with (potential) side
// effects is not evaluated: clang emits nothing for it and warns
// [-Wassume] (spec/landed/sema/WP-805.md). An argument without side
// effects still becomes an assumption. WP-805.
//@ expect: run
//@ strict-diags
//@ diag: 18:20 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 19:20 warning "assumption is ignored because it contains (potential) side-effects"
//@ diag: 20:20 warning "assumption is ignored because it contains (potential) side-effects"
//@ stdout: i=0 calls=0 v=5
extern "C" int printf(const char *, ...);

int calls = 0;
int g() { return ++calls; }

int main() {
  int i = 0, v = 5;
  __builtin_assume(++i > 0);
  __builtin_assume(g() != 0);
  __builtin_assume((i = 3) == 3);
  __builtin_assume(v == 5);
  printf("i=%d calls=%d v=%d\n", i, calls, v);
}
