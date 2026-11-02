// clang's checks on nullability qualifiers (spec/sema.md §4.1.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:5 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int'"
//@ diag: 20:5 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'int'"
//@ diag: 21:5 error "nullability specifier '_Null_unspecified' cannot be applied to non-pointer type 'int'"
//@ diag: 22:5 error "nullability specifier '_Nullable_result' cannot be applied to non-pointer type 'int'"
//@ diag: 23:6 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int &'"
//@ diag: 24:15 warning "duplicate nullability specifier '_Nonnull'"
//@ diag: 25:15 error "nullability specifier '_Nullable' conflicts with existing specifier '_Nonnull'"
//@ diag: 27:4 error "nullability specifier '_Nullable' conflicts with existing specifier '_Nonnull'"
//@ diag: 26:23 note "'_Nonnull' specified here"
//@ diag: 29:1 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int'; did you mean to apply the specifier to the pointer?"
//@ diag: 30:1 error "nullability specifier '_Nullable' cannot be applied to non-pointer type 'int'; did you mean to apply the specifier to the function pointer?"
//@ diag: 32:1 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int'; did you mean to apply the specifier to the member pointer?"
//@ diag: 33:1 error "nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int'"
//@ diag: 37:14 error "no matching function for call to 'g'"
//@ diag: 36:25 note "candidate template ignored: substitution failure [with T = int]: nullability specifier '_Nonnull' cannot be applied to non-pointer type 'int'"
int _Nonnull x1;
int _Nullable x2;
int _Null_unspecified x3;
int _Nullable_result x4;
int& _Nonnull r1 = x1;
int* _Nonnull _Nonnull dup;
int* _Nonnull _Nullable conflict;
typedef int* _Nonnull NP;
NP _Nullable q1;
NP _Nonnull q2;
_Nonnull int* moved;
_Nullable int (*mfp)(int);
struct S { int m; };
_Nonnull int S::* mmp;
_Nonnull int arr[2];

// A dependent type that becomes a non-pointer is a substitution failure.
template <class T> void g(T _Nonnull t);
void use() { g(1); }
