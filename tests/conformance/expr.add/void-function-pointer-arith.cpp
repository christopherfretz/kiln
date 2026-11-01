// [expr.add]/1: in C++ arithmetic on a pointer to void or to a function type is
// an error, not the GNU C extension (WP-219)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:12 error "arithmetic on a pointer to void"
//@ diag: 15:12 error "arithmetic on a pointer to void"
//@ diag: 16:12 error "arithmetic on pointers to void"
//@ diag: 17:4 error "arithmetic on a pointer to void"
//@ diag: 18:5 error "arithmetic on a pointer to void"
//@ diag: 19:13 error "arithmetic on a pointer to the function type 'void ()'"
//@ diag: 20:13 error "arithmetic on pointers to the function type 'void ()'"
//@ diag: 21:5 error "arithmetic on a pointer to the function type 'void ()'"
void g(void *p, void (*fp)(), int i) {
  (void)(p + 1);
  (void)(1 + p);
  (void)(p - p);
  p++;
  p += 2;
  (void)(fp + 1);
  (void)(fp - fp);
  fp++;
}
