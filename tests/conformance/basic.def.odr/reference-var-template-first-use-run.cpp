// [basic.def.odr]/5.1, [temp.inst]/7, [class.temporary]/6: the expression
// that first names a reference variable template specialization (and so
// instantiates its definition) odr-uses it and reads through the
// reference; later namings are constant uses folded to the extended
// temporary. Both designate the same object: a write through one is seen
// through the other, for a namespace-scope and a static member variable
// template, a constant and a dynamic initializer (WP-3290; clang emits the
// reference global for the first naming and loads through it).
//@ expect: run
//@ strict-diags
//@ stdout: 3 4 5 1
//@ stdout: 10 11 1
//@ stdout: 7 9 1
extern "C" int printf(const char *, ...);
int g(int x) { return x * 10; }
struct S { int a[2]; };

template <class T> S &&vm = S{{3, 4}};
template <class T> S &&vn = S{{g(1), 2}};
struct C {
  template <class T> static S &&sm;
};
template <class T> S &&C::sm = S{{7, 8}};

int *first_vm() { return &vm<int>.a[0]; }
int later_vm() { return vm<int>.a[1]; }
void bump_vm() { vm<int>.a[1] += 1; }
int first_vn() { return vn<int>.a[0]; }
void bump_vn() { vn<int>.a[0] += 1; }
int *later_vn() { return &vn<int>.a[0]; }
int first_sm() { return C::sm<int>.a[0]; }
void bump_sm() { C::sm<int>.a[1] += 1; }
int *later_sm() { return &C::sm<int>.a[1]; }

int main() {
  int *p = first_vm();
  int a = *p;
  int b = later_vm();
  bump_vm();
  int c = later_vm();
  printf("%d %d %d %d\n", a, b, c, p == &vm<int>.a[0]);
  int d = first_vn();
  bump_vn();
  int *q = later_vn();
  printf("%d %d %d\n", d, *q, q == &vn<int>.a[0]);
  int e = first_sm();
  bump_sm();
  int *r = later_sm();
  printf("%d %d %d\n", e, *r, r == &C::sm<int>.a[1]);
}
