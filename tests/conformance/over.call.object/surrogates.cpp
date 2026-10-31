// [over.call.object]/2: a conversion function to a pointer or reference to
// function adds a surrogate call function candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:22 error "no matching function for call to object of type 'B'"
//@ diag: 27:12 note "conversion candidate of type 'int (&)(int)'"
//@ diag: 27:27 note "conversion candidate of type 'int (*)(int, int)'"
//@ diag: 30:22 error "no matching function for call to object of type 'B'"
//@ diag: 27:12 note "conversion candidate of type 'int (&)(int)'"
//@ diag: 27:27 note "conversion candidate of type 'int (*)(int, int)'"
//@ diag: 32:22 error "call to object of type 'C' is ambiguous"
//@ diag: 31:16 note "candidate function"
//@ diag: 31:34 note "conversion candidate of type 'int (*)(int)'"
//@ diag: 34:22 error "no matching function for call to object of type 'D'"
//@ diag: 33:27 note "conversion candidate of type 'int (*)(int, int)'"
//@ diag: 33:12 note "conversion candidate of type 'int (*)(int)'"
//@ diag: 36:22 error "type 'E' does not provide a call operator"
//@ diag: 38:22 error "call to object of type 'H' is ambiguous"
//@ diag: 37:12 note "conversion candidate of type 'int (*)(int)'"
//@ diag: 37:27 note "conversion candidate of type 'int (*)(long)'"
//@ diag: 40:22 error "no matching function for call to object of type 'J'"
//@ diag: 39:12 note "conversion candidate of type 'int (*)(int)'"
typedef int F(int);
typedef int G(int, int); typedef int FL(long);
struct A { operator F*(); };
int s1(A a) { return a(1); }
struct B { operator F&(); operator G*() const; };
int s2(B b) { return b(1) + b(1, 2); }
int s3(B b) { return b(); }
int s4(B b) { return b("x"); }
struct C { int operator()(long); operator F*(); };
int s5(C c) { return c(1) + c(1L); }
struct D { operator F*(); operator G*(); };
int s6(D d) { return d(1, 2, 3); }
struct E { explicit operator F*(); };
int s7(E e) { return e(1); }
struct H { operator F*(); operator FL*();  };
int s8(H h) { return h(1.0); }
struct J { operator F*() &&; };
int s9(J j) { return j(1); }
int s10() { return J{}(1); }
struct K : A {};
int s11(K k) { return k(2); }
typedef void V(...); struct L { operator V*(); };
void s12(L l) { l(1, 2, "a"); }
