// [dcl.enum]/2: an opaque-enum-declaration of an unscoped enumeration
// shall not omit the enum-base; [dcl.type.elab]/3: an elaborated enum
// specifier never declares a new name. Clang 19 and 22 (without
// -fms-compatibility) report each such forward reference as the error "ISO
// C++ forbids forward references to 'enum' types" at the name: `enum F;`,
// a first use `enum K *`, a member, a parameter, a typedef, a friend, a
// local declaration that hides an outer enumeration, and each repeated
// `enum Z;`. A class template reports it once, at its definition. Found
// by the clang negative-test run (WP-957), fixed by WP-1078 (WP-1080).
// Valid twin: forward-ref-enum-ok.cpp.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:6 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 31:6 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 32:17 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 33:13 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 34:14 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 35:36 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 38:20 warning "elaborated enum specifier cannot be declared as a friend"
//@ diag: 38:20 note "remove 'enum' to befriend an enum"
//@ diag: 38:25 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 39:21 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 40:22 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 42:17 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 43:6 error "ISO C++ forbids forward references to 'enum' types"
//@ diag: 44:6 error "ISO C++ forbids forward references to 'enum' types"

enum F;
enum F { f1 };
enum K *pk;
struct S { enum L; };
void f(enum N *);
typedef enum T1 T1t;
template <class T> struct X { enum Q; enum Q *q; };
X<int> xi;
X<long> xl;
struct Fr { friend enum R; };
namespace NS { enum V *pv; }
int sz = sizeof(enum U *);
enum J { j };
void g() { enum J; enum J *p; }
enum Z;
enum Z;
enum Z *pz;
