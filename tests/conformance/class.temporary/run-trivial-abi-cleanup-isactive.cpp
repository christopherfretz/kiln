// [class.temporary]/3, [except.ctor]/3 with `[[clang::trivial_abi]]`: a
// callee-destroyed argument's caller cleanup is deactivated at the call,
// while later arguments' cleanups stay above it (clang keeps it in an
// `alloca i1 "cleanup.isactive"` flag). Whatever throws, each object is
// destroyed exactly once: by the caller when a later argument's
// construction or evaluation throws, by the callee once the call has
// started (also when the callee itself throws). WP-819; verified
// against clang 19 and clang 22.
//@ expect: run
//@ stdout: 1:Pd|2:BPd|3:BPd|4:fPB|5:fP|ok
extern "C" int printf(const char *, ...);
static char buf[16];
static int idx, live;
static void flush(int n) {
  buf[idx] = 0;
  printf("%d:%s|", n, buf);
  idx = 0;
}
struct [[clang::trivial_abi]] P {
  char id;
  explicit P(char c) : id(c) { ++live; }
  P(P &&o) : id(o.id) { ++live; o.id = 'm'; }
  ~P() { buf[idx++] = id; --live; }
};
struct B {
  char id;
  explicit B(char c, bool fail = false) : id(c) {
    if (fail) throw 1;
    ++live;
  }
  ~B() { buf[idx++] = id; --live; }
};
static_assert(__can_pass_in_regs(P));
static bool raise_in_callee;
__attribute__((noinline)) void f(P, B) {
  buf[idx++] = 'f';
  if (raise_in_callee) throw 2;
}
__attribute__((noinline)) void f3(P, B, int) { buf[idx++] = 'f'; }
__attribute__((noinline)) void g(P) {
  buf[idx++] = 'f';
  throw 3;
}
static int thrower() { throw 4; }
int main() {
  // 1: the second argument's constructor throws.
  try { f(P('P'), B('B', true)); } catch (int) { buf[idx++] = 'd'; }
  flush(1);
  // 2: a third argument throws after the second is constructed.
  try { f3(P('P'), B('B'), thrower()); } catch (int) { buf[idx++] = 'd'; }
  flush(2);
  // 3: the third argument is a throw-expression.
  try { f3(P('P'), B('B'), (throw 5, 0)); } catch (int) { buf[idx++] = 'd'; }
  flush(3);
  // 4: the callee throws after taking ownership of P.
  raise_in_callee = true;
  try { f(P('P'), B('B')); } catch (int) { }
  flush(4);
  // 5: a single callee-destroyed argument, callee throws.
  try { g(P('P')); } catch (int) { }
  flush(5);
  printf(live == 0 ? "ok\n" : "leak %d\n", live);
}
