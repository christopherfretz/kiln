// [class.mi]/4, [basic.align]/1: a class D with a virtual base V of
// stricter alignment is aligned as V only as a complete object; as a base
// subobject only its non-virtual part's alignment is guaranteed. clang
// gives an object reached through a `D*` the non-virtual alignment
// (`getClassPointerAlignment`, via `EmitPointerWithAlignment`). kiln took
// the complete-object alignment for `p->m`, `*p` and `p[i]` (WP-829): here
// it claimed `align 16` for `d->a`, but `D` is a non-primary base at offset
// 8 of `MD`, so `d->a` is 8 mod 16, and at -O2 the vectorized copy became
// an aligned vector load/store (x86-64 `movaps`, which faults on a
// misaligned address; AArch64 tolerates it, so only x86-64 crashed).
//@ expect: run
//@ args: -O2
//@ strict-diags
//@ stdout: 1.5 2.5 1
struct alignas(32) V { int v = 0; };
struct D : virtual V { long pad = 0; double a[4] = {0, 0, 0, 0}; };
struct X { virtual void f() {} };  // primary base: D lands at offset 8
struct MD : X, D {};

__attribute__((noinline)) void copy(D *d, const D *s) {
  double t0 = s->a[0], t1 = s->a[1], t2 = s->a[2], t3 = s->a[3];
  d->a[0] = t0;
  d->a[1] = t1;
  d->a[2] = t2;
  d->a[3] = t3;
}

__attribute__((noinline)) void copy_deref(D *d, const D *s) {
  double t0 = (*s).a[0], t1 = (*s).a[1], t2 = s[0].a[2], t3 = s[0].a[3];
  (*d).a[0] = t0 + 1;
  (*d).a[1] = t1 + 1;
  d[0].a[2] = t2 + 1;
  d[0].a[3] = t3 + 1;
}

extern "C" int printf(const char *, ...);

int main() {
  MD m1, m2;
  m2.a[0] = 0.5;
  m2.a[1] = 1.5;
  copy(&m1, &m2);
  copy_deref(&m2, &m1);
  D *p = &m2;
  bool off = (reinterpret_cast<unsigned long>(&p->a) % 16) != 0;
  printf("%.1f %.1f %d\n", m2.a[0], m2.a[1], off);
}
