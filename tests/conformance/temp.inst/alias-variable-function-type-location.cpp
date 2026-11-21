// [dcl.fct]/11, [temp.inst]/3: a member alias-declaration's or a variable
// template's function type that returns an array after substitution is
// diagnosed where the type-id's (declared type's) first type-specifier
// is, past a leading attribute or cv-qualifier (clang rebases the
// substituted `TypeSourceInfo` at its `TypeLoc` begin), not at the alias
// name or the variable. Reduced from real code. WP-1839.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:47 error "function cannot return array type 'const int[2]'"
//@ diag: 20:1 note "in instantiation of template class 'S<int[2]>' requested here"
//@ diag: 21:65 error "function cannot return array type 'int[2]'"
//@ diag: 22:1 note "in instantiation of template class 'S2<int[2]>' requested here"
//@ diag: 23:71 error "function cannot return array type 'const int[2]'"
//@ diag: 24:1 note "in instantiation of template class 'S3<int[2]>' requested here"
//@ diag: 25:26 error "function cannot return array type 'const int[2]'"
//@ diag: 26:19 note "in instantiation of variable template specialization 'vt' requested here"
//@ diag: 27:43 error "function cannot return array type 'int[2]'"
//@ diag: 28:20 note "in instantiation of variable template specialization 'vt2' requested here"
template <class T> struct S { using F = const T(); };
S<int[2]>::F *x;
template <class T> struct S2 { using F = __attribute__((cdecl)) T(); };
S2<int[2]>::F *x2;
template <class T> struct S3 { using F = __attribute__((cdecl)) const T (*)(); };
S3<int[2]>::F x3;
template <class T> const T (*vt)() = nullptr;
void uv() { (void)vt<int[2]>; }
template <class T> __attribute__((cdecl)) T (*vt2)();
void uv2() { (void)vt2<int[2]>; }
