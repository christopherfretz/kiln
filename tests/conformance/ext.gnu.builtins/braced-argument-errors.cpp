// A braced-init-list argument to a builtin with custom type checking is
// an initializer list of type `void` until something initializes from
// it ([dcl.init.list]/1), as in clang: each builtin's check rejects it
// by that type (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:19 error "cannot take the address of an rvalue of type 'void'"
//@ diag: 15:37 error "argument type 'void' is not a real floating point type"
//@ diag: 16:38 error "floating point classification requires argument of floating point type (passed in 'void')"
//@ diag: 17:19 error "non-pointer argument to '__builtin_launder' is not allowed"
//@ diag: 18:19 error "address argument to atomic builtin must be a pointer ('void' invalid)"

int i; int *p = &i;
void t1() { (void)__builtin_addressof({i}); }
void t2() { (void)__builtin_complex({1.0}, 2.0); }
void t4() { (void)__builtin_isfinite({1.0}); }
void t5() { (void)__builtin_launder({p}); }
void t6() { (void)__atomic_load_n({p}, 0); }
