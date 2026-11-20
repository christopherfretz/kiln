// [expr.new]/19, [over.ics.list]: a braced placement argument that
// cannot copy-list-initialize the parameter (too many initializers, an
// explicit constructor) makes the allocation function not viable: "no
// matching function for call to 'operator new'" with "cannot convert
// initializer list argument" candidate notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:9 error "no matching function for call to 'operator new'"
//@ diag: 20:7 note "candidate function not viable: cannot convert initializer list argument to 'P'"
//@ diag: 21:7 note "candidate function not viable: requires 3 arguments, but 2 were provided"
//@ diag: 25:9 error "no matching function for call to 'operator new'"
//@ diag: 21:7 note "candidate function not viable: cannot convert initializer list argument to 'Q'"
//@ diag: 20:7 note "candidate function not viable: requires 2 arguments, but 3 were provided"
//@ diag: 22:41 error "no matching function for call to 'operator new'"
//@ diag: 26:9 note "in instantiation of function template specialization 'make<int>' requested here"
//@ diag: 20:7 note "candidate function not viable: cannot convert initializer list argument to 'P'"
//@ diag: 21:7 note "candidate function not viable: requires 3 arguments, but 2 were provided"
struct P { int a, b; };
struct Q { explicit Q(int); };
void* operator new(decltype(sizeof 0) n, P p);
void* operator new(decltype(sizeof 0) n, int k, Q q);
template<class T> T* make(T v) { return new ({v, v, v}) T(v); }
void f() {
  (void)new ({1, 2, 3}) int(3);
  (void)new (1, {2}) int(4);
  (void)make(1);
}
