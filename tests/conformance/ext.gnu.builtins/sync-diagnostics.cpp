// GCC "Legacy __sync Built-in Functions" (clang
// `Sema::BuiltinAtomicOverloaded`): the first argument must be a pointer to
// a non-const integer or pointer of 1, 2, 4, 8 or 16 bytes; the operation
// needs its fixed arguments (extra ones are ignored), which are
// copy-initialized to the pointee type; the nand forms warn that GCC 4.4
// changed their meaning; and a `__sync` builtin is not a function value.
// BDE's bsls_atomicoperations_x64_all_gcc.h calls these on `int`,
// `bsls::Types::Int64` and `void *`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:24 error "too few arguments to function call, expected at least 1, have 0"
//@ diag: 30:25 error "too few arguments to function call, expected at least 2, have 1"
//@ diag: 31:3 error "address argument to atomic builtin must be a pointer ('int' invalid)"
//@ diag: 32:3 error "address argument to atomic builtin cannot be const-qualified ('const int *' invalid)"
//@ diag: 33:3 error "address argument to atomic builtin must be a pointer to integer or pointer ('float *' invalid)"
//@ diag: 34:3 error "address argument to atomic builtin must be a pointer to integer or pointer ('S *' invalid)"
//@ diag: 35:29 warning "the semantics of this intrinsic changed with GCC version 4.4 - the newer semantics are provided here"
//@ diag: 36:29 warning "the semantics of this intrinsic changed with GCC version 4.4 - the newer semantics are provided here"
//@ diag: 37:35 error "too few arguments to function call, expected at least 3, have 2"
//@ diag: 38:3 error "address argument to atomic builtin must be a pointer to integer or pointer ('char (*)[3]' invalid)"
//@ diag: 40:27 error "cannot initialize a parameter of type 'int' with an lvalue of type 'S *'"
//@ diag: 43:28 error "cannot initialize a parameter of type 'void *' with an rvalue of type 'int'"
//@ diag: 45:22 error "too many arguments to function call, expected 0, have 1"
//@ diag: 47:12 error "builtin functions must be directly called"

struct S { int x; };
void f(int *p, const int *cp, float *fp, S *sp, int i, bool *bp,
       char (*ap)[3], void **vp, __int128 *qp, volatile long *lp) {
  __sync_fetch_and_add();
  __sync_fetch_and_add(p);
  __sync_fetch_and_add(i, 1);
  __sync_fetch_and_add(cp, 1);
  __sync_fetch_and_add(fp, 1);
  __sync_fetch_and_add(sp, 1);
  __sync_fetch_and_nand(p, 1);
  __sync_nand_and_fetch(p, 1);
  __sync_val_compare_and_swap(p, 1);
  __sync_fetch_and_add(ap, 1);
  __sync_fetch_and_add(p, 1, 2, 3);       // OK: extra arguments ignored
  __sync_fetch_and_add(p, sp);
  __sync_lock_release(p, 5);              // OK
  __sync_fetch_and_add(bp, 1);            // OK
  __sync_fetch_and_add(vp, 1);
  __sync_fetch_and_add(qp, 1);            // OK
  __sync_synchronize(1);
  __sync_fetch_and_add(lp, 1);            // OK
  auto x = __sync_fetch_and_add;
}
