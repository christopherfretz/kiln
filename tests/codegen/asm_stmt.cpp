// [dcl.asm] (GNU extended asm): an asm statement lowers to a call of an
// LLVM inline-asm value with clang's constraint string (outputs, inputs,
// the input halves of "+" outputs, clobbers, target clobbers), `elementtype`
// on indirect operands, `memory(none)`/`memory(read)` for non-volatile asm
// with outputs, and a `!srcloc` per template line; a namespace-scope asm
// becomes `module asm`.
asm(".globl kiln_asm_sym\n.set kiln_asm_sym, 0");

template <class T> void do_not_optimize(const T &value) { asm volatile("" : : "r,m"(value) : "memory"); }
template <class T> void do_not_optimize(T &value) { asm volatile("" : "+r,m"(value) : : "memory"); }
template <class T> void do_not_optimize_m(T &value) { asm volatile("" : "+m,r"(value) : : "memory"); }

struct S4 { short a, b; };
struct S12 { int a, b, c; };

void basic() {
  asm("nop\n\tnop $");
  asm volatile("nop");
}

long outputs(long x, long y) {
  long r, s;
  asm("mov %0, %2\n\tmov %1, %3" : "=r"(r), "=&r"(s) : "r"(x), "r"(y));
  asm("" : "=r"(r) : "m"(x));
  asm("" : "+r"(r));
  asm("" : "=m"(s) : "0"(x));
  return r + s;
}

int tied(char c, long l) {
  long wide;
  asm("" : "=r"(wide) : "0"(c));
  int narrow;
  asm("" : "=r"(narrow) : "0"(l));
  bool b;
  asm("" : "=r"(b));
  return int(wide) + narrow + b;
}

void aggregates(S4 &s4, S12 &s12) {
  asm volatile("" : "+r"(s4));
  asm volatile("" :: "r"(s4), "m"(s12));
  asm volatile("" : "=m"(s12));
}

void immediates(long *p) {
  asm volatile("# %0 %1" :: "i"(42), "n"(sizeof(long)));
  asm volatile("" :: "r"(p) : "cc", "memory");
}

void use() {
  int i = 0;
  const int ci = 1;
  double d = 2.0;
  do_not_optimize(i);
  do_not_optimize(ci);
  do_not_optimize(d);
  do_not_optimize_m(i);
}

void clobbers() {
#ifdef __aarch64__
  asm volatile("" ::: "x29", "r3", "0", "memory");
#else
  asm volatile("" ::: "rax", "0", "eax", "cc", "memory");
#endif
}

void multiline(int x) {
  asm volatile("add %w0, %w0, #1\n"
               "sub %w0, %w0, #1" : "+r"(x));
}
