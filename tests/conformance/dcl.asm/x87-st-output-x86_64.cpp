// [dcl.asm] GNU inline asm on x86-64: the x87 top-of-stack letters "t"
// and "u" are valid outputs, plain or read-write, and "f" is a valid input
// (clang 22 and clang 19 accept them; only an "f" output is refused, see
// x87-read-write-output-x86_64.cpp).
//@ expect: compile-ok
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags

double f(double d, double e) {
  asm("fsqrt" : "=t"(d) : "0"(d));
  asm("fyl2x" : "=t"(d) : "0"(d), "u"(e) : "st(1)");
  asm("" : "+t"(d));
  asm("" : "+u"(e));
  asm("" : "=&t"(d) : "f"(e));
  return d + e;
}
