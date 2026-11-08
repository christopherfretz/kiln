// [dcl.attr.grammar]/5 clang's `pass_object_size(t)` and
// `pass_dynamic_object_size(t)` (AttrDocs "pass_object_size"): the hidden
// argument is clang `MaybeEmitImplicitObjectSize`'s, the constant fold of
// `__builtin_object_size(arg, t)` at the call site (any form the Oven
// folds: a subobject, an array element, an index past a member, a
// conditional, a string literal's designator), for the dynamic spelling
// too; else the caller's own hidden argument for a compatible parameter,
// else 0 for type 3, else `llvm.objectsize`. Member, static member,
// operator and constructor calls pass it as free calls do. spec/codegen.md
// "pass_object_size", WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
#define PS(t) __attribute__((pass_object_size(t)))
#define PD(t) __attribute__((pass_dynamic_object_size(t)))
void f0(const void *PS(0) p);
void f1(const void *PS(1) p);
void f2(const void *PS(2) p);
void f3(const void *PS(3) p);
void d0(const void *PD(0) p);
void d1(const void *PD(1) p);
struct In { char c[6]; int k; };
struct S { char a[10]; int arr[4]; In in; double d; };
S gs;
char gbuf[32];
char (&gref)[32] = gbuf;
void forms(int i, S *ps, bool b) {
  S s;
  char lb[8];
  f0(&s.arr[2]); f1(&s.arr[2]); f2(&s.arr[2]); f3(&s.arr[2]);
  f0(s.arr + 3); f1(s.arr + 3);
  f0(&s.in.c[1]); f1(&s.in.c[1]); f3(&s.in.c[1]);
  f0(&s.in); f1(&s.in);
  f0("literal"); f1("lit"); f3(&"lit"[1]);
  f0(gref); f1(gref);
  f0(&gs.in.k); f1(&gs.in.k);
  f0(b ? gbuf : lb); f0(true ? gbuf : lb);
  f0(&lb[i]); f1(&lb[i]); f3(&lb[i]);
  f0(ps);
  f0(&s.d); f1(&s.d);
  f0(&gbuf[32]); f1(&gbuf[32]);
  f0(static_cast<void *>(&s.arr[1])); f1(static_cast<void *>(&s.arr[1]));
  f0(&(&s)->in); f0(&*lb); f1(&*(lb + 2)); f0(&lb[1] + 2);
  f0((i, lb));
  d0(&s.arr[2]); d1(&s.arr[2]); d0(&lb[i]); d1(&s.in.c[1]);
}
// The caller's own hidden argument (types equal, 0 serving 1, 3 serving 2).
void pass(char *const PS(0) p, char *const PS(3) q) { f0(p); f1(p); f2(q); f3(q); f0(p + 1); }
struct M {
  void m(const char *PS(1) p);
  static void st(const char *PS(0) p);
  void operator()(const char *PS(0) p);
  M(const char *PS(1) p);
};
struct Op {};
void operator+(Op, const char *PS(0) p);
void members(M &m) {
  char buf[8];
  m.m(buf); M::st(buf + 2); m(buf); Op() + buf;
  M x(buf + 1);
  M *h = new M(buf);
  (void)h;
}
