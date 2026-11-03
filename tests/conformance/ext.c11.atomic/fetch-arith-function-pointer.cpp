// Clang extension: C11 atomic fetch arithmetic on a pointer requires a
// complete pointee ([basic.types.general]/5), but a function type is not
// incomplete, so `_Atomic(void (*)(int))` reports nothing there (the libc++
// atomic_fetch_add/sub .verify tests then see only their static_assert);
// `void` and an incomplete class pointee are still errors.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:26 error "incomplete type 'Incomplete' where a complete type is required"
//@ diag: 12:8 note "forward declaration of 'Incomplete'"
//@ diag: 19:26 error "incomplete type 'void' where a complete type is required"
void f(int);
struct Incomplete;
void use(_Atomic(void (*)(int)) *fp, _Atomic(Incomplete *) *ip,
         _Atomic(void *) *vp) {
  __c11_atomic_fetch_add(fp, 1, 5);
  __c11_atomic_fetch_sub(fp, 1, 5);
  __c11_atomic_fetch_add(fp, 1, 5);
  __c11_atomic_fetch_add(ip, 1, 5);
  __c11_atomic_fetch_sub(vp, 1, 5);
}
