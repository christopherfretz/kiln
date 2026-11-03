// [class.temporary]/3 with clang's `[[clang::trivial_abi]]`: a class with
// the attribute in effect is trivial for the purposes of calls, so a
// by-value parameter of it is destroyed by the callee at its end, not by
// the caller at the end of the full-expression (the other arguments still
// are, in reverse order of construction); a class with such a member is
// too. If a later argument's evaluation throws, the caller destroys the
// already-constructed argument ([except.ctor]/3). Verified against clang
// 19 and clang 22.
//@ expect: run
//@ stdout: fBCA|gw|BA|hQm
extern "C" int printf(const char *, ...);
static char buf[16];
static int idx;
static void flush(const char *sep) {
  buf[idx] = 0;
  printf("%s%s", buf, sep);
  idx = 0;
}
struct Base {
  char id;
  explicit Base(char c) : id(c) {}
  Base(const Base &) = default;
  ~Base() { buf[idx++] = id; }
};
struct A : Base { A() : Base('A') {} };
struct C : Base { C() : Base('C') {} };
struct [[clang::trivial_abi]] P {
  long v;
  char id;
  explicit P(char c) : v(0), id(c) {}
  P(P &&o) : v(o.v), id(o.id) { o.id = 'm'; }
  ~P() { buf[idx++] = id; }
};
struct W { P p; };
static_assert(__can_pass_in_regs(P));
static_assert(__can_pass_in_regs(W));
__attribute__((noinline)) void f(A, P, C) { buf[idx++] = 'f'; }
__attribute__((noinline)) void g(W) { buf[idx++] = 'g'; }
__attribute__((noinline)) void h(P p) { buf[idx++] = 'h'; }
int main() {
  f(A(), P('B'), C());
  flush("|");
  g(W{P('w')});
  flush("|");
  try {
    f(A(), P('B'), (throw 1, C()));
  } catch (int) {
  }
  flush("|");
  {
    P q('Q');
    h(static_cast<P &&>(q));
  }
  flush("\n");
}
