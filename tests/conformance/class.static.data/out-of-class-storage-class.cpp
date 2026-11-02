// [class.static.data]/2-3: the out-of-class definition of a static data
// member takes no storage class. clang: "'static' can only be specified
// inside the class definition" for `static`, "static data member
// definition cannot specify a storage class" for `extern` and `register`
// (after -Wregister), at the specifier; the definition stays valid
// (WP-327). `mutable` is dcl.stc/mutable-nonmember.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:1 error "'static' can only be specified inside the class definition"
//@ diag: 21:1 error "static data member definition cannot specify a storage class"
//@ diag: 22:1 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 22:1 error "static data member definition cannot specify a storage class"
//@ diag: 23:1 error "static data member definition cannot specify a storage class"
//@ diag: 25:1 error "'static' can only be specified inside the class definition"
struct S {
  static int m; static int n; static int q;
  static const int c;
};
struct T { static thread_local int z; };
static int S::m = 0;
extern int S::n;
register int S::q = 2;
extern const int S::c = 3;
static_assert(S::c == 3);
static thread_local int T::z = 1;
int use = S::m + S::n + S::q + T::z;
