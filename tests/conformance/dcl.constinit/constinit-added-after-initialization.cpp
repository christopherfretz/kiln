// [dcl.constinit]/1: if `constinit` appears on any declaration of a
// variable, it appears on the initializing declaration. A redeclaration
// that adds it after the variable was initialized (or defined) is
// ill-formed: clang's "'constinit' specifier added after initialization of
// variable" at the specifier, noted at the initializing declaration (the
// one with an initializer, else the definition), and the specifier is
// dropped. At namespace and block scope, for a static data member whose
// in-class declaration has the initializer, a variable template and a
// thread_local variable; not after an earlier `constinit` or
// `require_constant_initialization`, not on an explicit specialization (a
// variable of its own), and not on the initializing declaration itself
// (there, an earlier `constinit` only warns, -Wmissing-constinit). Same in
// C++20, C++23 and C++26 and at clang 19. spec/sema.md §1.1, WP-1834.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:8 error "'constinit' specifier added after initialization of variable"
//@ diag: 41:5 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 44:8 error "'constinit' specifier added after initialization of variable"
//@ diag: 43:5 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 46:8 error "'constinit' specifier added after initialization of variable"
//@ diag: 45:12 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 48:8 error "'constinit' specifier added after initialization of variable"
//@ diag: 47:12 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 51:1 error "'constinit' specifier added after initialization of variable"
//@ diag: 50:5 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 53:8 error "'constinit' specifier added after initialization of variable"
//@ diag: 52:18 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 54:19 error "'constinit' specifier added after initialization of variable"
//@ diag: 41:5 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 56:1 error "'constinit' specifier added after initialization of variable"
//@ diag: 55:29 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 60:27 error "'constinit' specifier added after initialization of variable"
//@ diag: 59:24 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 62:3 warning "'require_constant_initialization' attribute added after initialization of variable"
//@ diag: 61:5 note "add the 'require_constant_initialization' attribute to the initializing declaration here"
//@ diag: 63:1 error "'constinit' specifier added after initialization of variable"
//@ diag: 61:5 note "add the 'constinit' specifier to the initializing declaration here"
//@ diag: 65:8 error "'constinit' specifier added after initialization of variable"
//@ diag: 64:5 note "add the 'constinit' specifier to the initializing declaration here"
int a1 = 1;
extern constinit int a1;
int a2;
extern constinit int a2;
static int a3 = 1;
extern constinit int a3;
inline int a4 = 1;
extern constinit int a4;
extern int a5;
int a5 = 1;
constinit extern int a5;
thread_local int a6 = 1;
extern constinit thread_local int a6;
void f() { extern constinit int a1; }
struct S { static const int x = 1; static int y; static const int z; };
constinit const int S::x;
int S::y = 1;
const int S::z = 1;
template <class T> int vt = 0;
template <class T> extern constinit int vt;
int a7 = 1;
[[clang::require_constant_initialization]] extern int a7;
constinit extern int a7;
int a8 = 1;
extern constinit int a8, a9;

// Accepted.
extern constinit int k1;
constinit int k1 = 1;
extern constinit int k1;
extern int k2;
constinit int k2 = 2;
extern constinit int k2;
[[clang::require_constant_initialization]] int k3 = 3;
extern constinit int k3;
template <class T> struct TT { static int s; };
template <class T> int TT<T>::s = 1;
template <> constinit int TT<int>::s;
struct M { static constinit int m; };
constinit int M::m = 4;
extern constinit int k4;
extern int k4;
int k5 = 1;
extern int k5;
