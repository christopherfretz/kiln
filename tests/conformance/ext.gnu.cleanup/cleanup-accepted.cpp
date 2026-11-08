// GNU `cleanup(f)` parameters both clang 19 and 22 accept (WP-807; C's
// assignment rules against `T *`): `bool`, `void *`, a more qualified
// pointee, an `enum` or a typedef of `T`, a reference to `T *`, and a
// template's non-dependent local. No warning is expected.
//@ expect: compile-ok
//@ strict-diags
enum E { e0 };
typedef int I;
void fb(bool);
void fv(void *);
void fcv(const volatile void *);
void fc(const int *);
void fe(E *);
void fi(I *);
void fref(int *&);
void f(int *);
template <class T> void tl() {
  int a __attribute__((cleanup(f)));
}
void use() {
  int a __attribute__((cleanup(fb)));
  int b __attribute__((cleanup(fv)));
  int c __attribute__((cleanup(fcv)));
  int d __attribute__((cleanup(fc)));
  E e __attribute__((cleanup(fe)));
  int g __attribute__((cleanup(fi)));
  int h __attribute__((cleanup(fref)));
  tl<int>();
}
