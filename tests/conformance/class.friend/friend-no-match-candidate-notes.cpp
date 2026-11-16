// [class.friend]/4, [dcl.meaning.general]/2: a qualified friend that
// matches no member notes each candidate the name finds, as clang does:
// why its type differs, implicit special members by kind, and the
// function templates (no specialization matches) last (WP-1495).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 23:13 error "friend declaration of 'A' does not match any declaration in 'A'"
//@ diag: 21:12 note "candidate constructor has type mismatch at 1st parameter (expected 'double' but has 'int')"
//@ diag: 21:8 note "candidate constructor (the implicit copy constructor) has type mismatch at 1st parameter (expected 'double' but has 'const A &')"
//@ diag: 21:8 note "candidate constructor (the implicit move constructor) has type mismatch at 1st parameter (expected 'double' but has 'A &&')"
//@ diag: 21:38 note "candidate template ignored: failed template argument deduction"
//@ diag: 24:18 error "friend declaration of 'f' does not match any declaration in 'A'"
//@ diag: 21:52 note "candidate function has type mismatch at 1st parameter (expected 'double' but has 'int')"
//@ diag: 21:98 note "candidate function has different number of parameters (expected 1 but has 2)"
//@ diag: 21:83 note "candidate template ignored: failed template argument deduction"
//@ diag: 27:22 error "friend declaration of 'C' does not match any declaration in 'C'"
//@ diag: 26:12 note "candidate constructor has different number of parameters (expected 1 but has 0)"
//@ diag: 26:17 note "candidate constructor has type mismatch at 1st parameter (expected 'int' but has 'C &&')"
//@ diag: 26:8 note "candidate constructor (the implicit copy constructor) has type mismatch at 1st parameter (expected 'int' but has 'const C &')"
struct A { A(int); template<class T> A(T, T); void f(int); template<class T> void f(T, int); int f(int, int) const; };
struct B {
  friend A::A(double);
  friend void A::f(double);
};
struct C { C(); C(C&&); };
struct D { friend C::C(int); };
