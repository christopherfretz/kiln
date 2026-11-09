// [dcl.type.general]/3, [dcl.fct]/17, [temp.param]/2: a class or enumeration
// definition may appear only in the decl-specifier-seq of a declaration (or
// a defining-type-specifier of an alias-declaration). clang's ActOnTag
// rejects one in a type-id: a non-type template parameter type, a
// type-parameter default, sizeof/alignof/typeid operands, casts,
// __builtin_offsetof, template arguments and decltype operands. A default
// argument or a noexcept operand of a function declarator lies in the
// prototype scope and names "a parameter type" instead.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:17 error "'S1' cannot be defined in a type specifier"
//@ diag: 33:27 error "'S2' cannot be defined in a type specifier"
//@ diag: 34:32 error "'S3' cannot be defined in a type specifier"
//@ diag: 35:24 error "'S4' cannot be defined in a type specifier"
//@ diag: 36:25 error "'S5' cannot be defined in a type specifier"
//@ diag: 37:42 error "'S6' cannot be defined in a type specifier"
//@ diag: 38:18 error "'S7' cannot be defined in a type specifier"
//@ diag: 39:28 error "'E1' cannot be defined in a type specifier"
//@ diag: 40:17 error "'(unnamed enum at "
//@ diag: 41:36 error "'S8' cannot be defined in a type specifier"
//@ diag: 42:18 error "'S9' cannot be defined in a type specifier"
//@ diag: 43:30 error "'S10' cannot be defined in a type specifier"
//@ diag: 44:34 error "'S11' cannot be defined in a parameter type"
//@ diag: 45:35 error "'Y::S12' cannot be defined in a type specifier"
//@ diag: 46:34 error "'S13' cannot be defined in a type specifier"
//@ diag: 47:51 error "'S14' cannot be defined in a type specifier"
//@ diag: 49:17 error "'S15' cannot be defined in a type specifier"
//@ diag: 50:29 error "'S16' cannot be defined in a type specifier"
//@ diag: 51:24 error "'S17' cannot be defined in a type specifier"
namespace std { class type_info; }
template<struct S1 { int a; } *P> struct N1 {};
template<class T = struct S2 { int b; }> struct N2 {};
template<int N = sizeof(struct S3 { int c; })> struct N3 {};
int g1 = sizeof(struct S4 { int d; });
int g2 = alignof(struct S5 { int e; });
const std::type_info &g3 = typeid(struct S6 { int f; });
int g4 = (struct S7 { operator int(); }) 0;
int g5 = (int) sizeof(enum E1 { a1 });
int g6 = sizeof(enum { a2 });
int g7 = __builtin_offsetof(struct S8 { int g; }, g);
int g8 = (struct S9 { int h; } *)0 == 0;
int g9 = sizeof(const struct S10 { int i; } *);
void f1() noexcept(sizeof(struct S11 { int j; }));
struct Y { explicit(sizeof(struct S12 { int k; }) == 4) Y(); };
enum E2 : decltype(sizeof(struct S13 { int l; })) {};
template<class T> void f2() { (void)sizeof(struct S14 { int m; }); }
template<int> struct Z {};
Z<sizeof(struct S15 { int n; })> z;
static_assert(sizeof(struct S16 { int o; }) == 4, "");
decltype(sizeof(struct S17 { int p; })) g10;
