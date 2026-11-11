// C17 6.3.2.3p5-6, 6.5.4: a cast between a pointer and an integer of any width
// is valid C (implementation-defined). clang warns -Wpointer-to-int-cast for a
// pointer cast to an integer narrower than a pointer (-Wvoid-pointer-to-int-cast
// from `void *`, -Wpointer-to-enum-cast to an enumeration) and
// -Wint-to-pointer-cast for a non-constant integer cast to a wider pointer; a
// `_Bool`, an enumeration or an integer constant expression is not diagnosed
// (clang `CheckCStyleCast`). Both CI targets are LP64. WP-1285.
//@ std: c17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 23:12 warning "cast to smaller integer type 'int' from 'void *'"
//@ diag: 24:12 warning "cast to smaller integer type 'int' from 'int *'"
//@ diag: 25:14 warning "cast to smaller integer type 'short' from 'void (*)(void)'"
//@ diag: 26:13 warning "cast to smaller integer type 'char' from 'int *'"
//@ diag: 28:15 warning "cast to smaller integer type 'enum E' from 'int *'"
//@ diag: 29:15 warning "cast to smaller integer type 'enum E' from 'void *'"
//@ diag: 31:13 warning "cast to 'int *' from smaller integer type 'short'"
//@ diag: 32:14 warning "cast to 'void *' from smaller integer type 'char'"
//@ diag: 36:22 warning "cast to 'void (*)(void)' from smaller integer type 'short'"
enum E { A };
void f(void *vp, int *ip, void (*fp)(void), long l, short s, _Bool b, enum E e,
       char c) {
  int i1 = (int)vp;
  int i2 = (int)ip;
  short s1 = (short)fp;
  char c1 = (char)ip;
  _Bool b1 = (_Bool)ip;
  enum E e1 = (enum E)ip;
  enum E e2 = (enum E)vp;
  unsigned long u = (unsigned long)ip;
  int *p1 = (int *)s;
  void *p2 = (void *)c;
  int *p3 = (int *)b;
  int *p4 = (int *)e;
  void *p5 = (void *)e;
  void (*f2)(void) = (void (*)(void))s;
  int *p6 = (int *)l;
  int *p8 = (int *)(char)1;
  void *p9 = (void *)'a';
  (void)i1, (void)i2, (void)s1, (void)c1, (void)b1, (void)e1, (void)e2;
  (void)u, (void)p1, (void)p2, (void)p3, (void)p4, (void)p5, (void)f2;
  (void)p6, (void)p8, (void)p9;
}
