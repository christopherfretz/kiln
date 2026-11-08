// GNU `cleanup(f)` on locals (WP-807): `f(&v)` on the normal exit and in
// an EH cleanup when a call may throw, before `v`'s destructor and in
// reverse declaration order; a `void *` parameter takes the address as
// is; a VLA's cleanup follows its stack save.
void f(int *);
void fv(void *);
void may_throw();
struct D { ~D(); };
void fd(D *);
void plain() {
  int a __attribute__((cleanup(f))) = 1;
  int b __attribute__((cleanup(fv))) = 2;
}
void eh() {
  D d __attribute__((cleanup(fd)));
  int a __attribute__((cleanup(f))) = 1;
  may_throw();
}
int ret(int x) {
  int a __attribute__((cleanup(f))) = x;
  if (x)
    return a;
  may_throw();
  return 0;
}
