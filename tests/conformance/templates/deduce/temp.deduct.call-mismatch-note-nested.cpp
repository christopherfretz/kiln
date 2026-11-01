// [temp.deduct.type]/1, [temp.deduct.call]/3: 'could not match' names the
// mismatched part of P as written at any depth, with P's cv intersected
// with A's (an array's cv is its element's).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:12 error "no matching function for call to 'f1'"
//@ diag: 31:24 note "candidate template ignored: could not match 'S<T>' against 'const char'"
//@ diag: 41:22 error "no matching function for call to 'f2'"
//@ diag: 32:24 note "candidate template ignored: could not match 'const S<T>' against 'const char'"
//@ diag: 41:32 error "no matching function for call to 'f3'"
//@ diag: 33:24 note "candidate template ignored: could not match 'T[4]' against 'const char[3]'"
//@ diag: 41:42 error "no matching function for call to 'f4'"
//@ diag: 34:24 note "candidate template ignored: could not match 'S<T>' against 'const char'"
//@ diag: 41:52 error "no matching function for call to 'f5'"
//@ diag: 35:24 note "candidate template ignored: could not match 'const S<T>' against 'const char'"
//@ diag: 41:62 error "no matching function for call to 'f6'"
//@ diag: 36:24 note "candidate template ignored: could not match 'S<T>' against 'const char[3]'"
//@ diag: 41:72 error "no matching function for call to 'f7'"
//@ diag: 37:24 note "candidate template ignored: could not match 'S<T>' against 'const char[3]'"
//@ diag: 41:82 error "no matching function for call to 'f8'"
//@ diag: 38:24 note "candidate template ignored: could not match 'S<T>' against 'const char[3]'"
//@ diag: 41:97 error "no matching function for call to 'f9'"
//@ diag: 39:24 note "candidate template ignored: could not match 'const S<T>' against 'const char[3]'"
//@ diag: 42:3 error "no viable constructor or deduction guide for deduction of template arguments of 'J'"
//@ diag: 40:26 note "candidate template ignored: could not match 'J<T>' against 'const char *'"
//@ diag: 40:26 note "implicit deduction guide declared as 'template <class T> J(J<T>) -> J<T>'"
//@ diag: 40:30 note "candidate template ignored: could not match 'const S<T>' against 'const char[3]'"
//@ diag: 40:30 note "implicit deduction guide declared as 'template <class T> J(const S<T> &) -> J<T>'"
//
template<class T> struct S { };
template<class T> void f1(S<T> (&)[3]);
template<class T> void f2(const S<T> (&)[3]);
template<class T> void f3(T (&)[4]);
template<class T> void f4(S<T>*);
template<class T> void f5(const S<T>*);
template<class T> void f6(volatile S<T>&);
template<class T> void f7(S<T>&&);
template<class T> void f8(T*, S<T>&);
template<class T> void f9(const S<T>&);
template<class T> struct J { J(const S<T>&); };
void z() { f1("ab"); f2("ab"); f3("ab"); f4("ab"); f5("ab"); f6("ab"); f7("ab"); f8("x", "ab"); f9("ab"); }
J j("ab");
