// [over.ics.list]/9: a braced argument for a reference parameter is converted as
// the referenced type. When that fails clang's note names the referenced type;
// when the temporary would initialize but a non-const lvalue reference cannot
// bind it, the note is 'expects an lvalue'.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:24 error "no matching function for call to 'h'"
//@ diag: 42:6 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 42:20 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 46:32 error "no matching function for call to 'h'"
//@ diag: 42:6 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 42:20 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 46:40 error "no matching function for call to 'h'"
//@ diag: 42:6 note "candidate function not viable: cannot convert initializer list argument to 'int'"
//@ diag: 42:20 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 46:51 error "no matching function for call to 'n'"
//@ diag: 43:6 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 43:18 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 46:62 error "no matching function for call to 'k'"
//@ diag: 44:6 note "candidate function not viable: cannot convert initializer list argument to 'H'"
//@ diag: 44:19 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 46:76 error "no matching function for call to 'm'"
//@ diag: 45:6 note "candidate function not viable: cannot convert initializer list argument to 'const H'"
//@ diag: 45:24 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 48:25 error "no matching function for call to 'q'"
//@ diag: 47:6 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 48:33 error "no matching function for call to 'q'"
//@ diag: 47:6 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 48:41 error "no matching function for call to 'q'"
//@ diag: 47:6 note "candidate function not viable: cannot convert initializer list argument to 'int'"
//@ diag: 48:52 error "no matching function for call to 'r'"
//@ diag: 47:20 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 48:63 error "no matching function for call to 's'"
//@ diag: 47:32 note "candidate function not viable: cannot convert initializer list argument to 'H'"
//@ diag: 48:77 error "no matching function for call to 't'"
//@ diag: 47:45 note "candidate function not viable: cannot convert initializer list argument to 'const H'"
//@ diag: 48:91 error "no matching function for call to 'u'"
//@ diag: 47:63 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 48:99 error "no matching function for call to 'q'"
//@ diag: 47:6 note "candidate function not viable: expects an lvalue for 1st argument"
struct H { int a; int b; };
void h(int&); void h(int&, int);
void n(H&); void n(H&, int);
void k(H&&); void k(H&&, int);
void m(const H&); void m(const H&, int);
void g() { long l = 0; h({1}); h({l}); h({1, 2}); n({1, 2}); k({1, 2, 3}); m({1, 2, 3}); }
void q(int&); void r(H&); void s(H&&); void t(const H&); void u(volatile H&);
void g2() { long l = 0; q({1}); q({l}); q({1, 2}); r({1, 2}); s({1, 2, 3}); t({1, 2, 3}); u({1}); q({}); }
