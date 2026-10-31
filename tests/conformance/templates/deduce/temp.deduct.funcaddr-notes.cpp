// [temp.deduct.funcaddr], [over.over]/5: when no function matches the
// target, non-template candidates are noted first, then the template
// deduction failures in source order.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:20 error "address of overloaded function 'f' does not match required type 'void (int)'"
//@ diag: 31:25 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 37:26 error "address of overloaded function 'g' does not match required type 'void (int, long)'"
//@ diag: 33:6 note "candidate function has different number of parameters (expected 2 but has 1)"
//@ diag: 32:25 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
//@ diag: 38:20 error "address of overloaded function 'h' does not match required type 'long (int)'"
//@ diag: 34:24 note "candidate template ignored: could not match 'int' against 'long'"
//@ diag: 43:25 error "address of overloaded function 'g2' does not match required type 'void (int, long)'"
//@ diag: 42:6 note "candidate function has different number of parameters (expected 2 but has 3)"
//@ diag: 40:6 note "candidate function has different number of parameters (expected 2 but has 1)"
//@ diag: 41:25 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
//@ diag: 47:25 error "address of overloaded function 'm' does not match required type 'void (int, long)'"
//@ diag: 45:6 note "candidate function has different number of parameters (expected 2 but has 1)"
//@ diag: 44:25 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
//@ diag: 46:25 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
//@ diag: 49:24 error "address of overloaded function 'm3' does not match required type 'void (int, int)'"
//@ diag: 48:25 note "candidate template ignored: failed template argument deduction"
//@ diag: 50:34 error "address of overloaded function 'm3' does not match required type 'void (int, int, int, int)'"
//@ diag: 48:25 note "candidate template ignored: failed template argument deduction"
//@ diag: 51:34 error "address of overloaded function 'm3' does not match required type 'void (int, int, int, ...)'"
//@ diag: 48:25 note "candidate template ignored: could not match 'void (T, T, T)' against 'void (int, int, int, ...)'"
//@ diag: 53:29 error "address of overloaded function 'n' does not match required type 'void (int, int, int)'"
//@ diag: 52:22 note "candidate template ignored: deduced conflicting types for parameter 'T' ('void' vs. 'int')"
//@ diag: 54:29 error "address of overloaded function 'n' does not match required type 'long (int, int, int)'"
//@ diag: 52:22 note "candidate template ignored: deduced conflicting types for parameter 'T' ('long' vs. 'int')"
template <class T> void f(T*);
template <class T> void g(T, T);
void g(int);
template <class T> int h(T);
void use() {
  void (*p)(int) = f;
  void (*q)(int, long) = g;
  long (*r)(int) = h;
}
void g2(int);
template <class T> void g2(T, T);
void g2(long, long, long);
void (*q2)(int, long) = g2;
template <class T> void m(T, T);
void m(int);
template <class T> void m(T, T, T);
void (*r2)(int, long) = m;
template <class T> void m3(T, T, T);
void (*p3)(int, int) = m3;
void (*q3)(int, int, int, int) = m3;
void (*r3)(int, int, int, ...) = m3;
template <class T> T n(T, T);
void (*s3)(int, int, int) = n;
long (*t3)(int, int, int) = n;
