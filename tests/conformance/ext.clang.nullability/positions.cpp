// clang's nullability qualifiers (a clang extension, spec/sema.md
// §4.1.1): `_Nonnull`, `_Nullable`, `_Null_unspecified` and
// `_Nullable_result` are type sugar on a pointer or member pointer,
// written after `*` or in the decl-specifier-seq; the canonical type is
// unchanged.
//@ expect: compile-ok
typedef int* _Nonnull p1;
typedef int* _Nullable p2;
typedef int* _Null_unspecified p3;
typedef int* _Nullable_result p4;
static_assert(__is_same(p1, int*) && __is_same(p2, int*));
static_assert(__is_same(p3, int*) && __is_same(p4, int*));

// Function parameters and return types.
void f1(int* _Nonnull a, int* _Nullable b, int* _Null_unspecified c,
        int* _Nullable_result d);
int* _Nonnull f2(const char* _Nullable s);
static_assert(__is_same(decltype(f1), void(int*, int*, int*, int*)));

// Member pointers, data and function.
struct S { int m; int g(); };
int S::* _Nonnull mp1 = &S::m;
int S::* _Nullable mp2 = nullptr;
int S::* _Null_unspecified mp3 = nullptr;
int (S::* _Nullable_result mf)() = &S::g;

// Nested pointers, each level qualified, with cv-qualifiers in either
// order.
int* _Nullable* _Nonnull np1 = nullptr;
int* _Nonnull* _Null_unspecified* _Nullable_result np2 = nullptr;
int* const _Nonnull cp1 = nullptr;
int* _Nullable const cp2 = nullptr;
static_assert(__is_same(decltype(np2), int***));
static_assert(__is_same(decltype(cp1), int* const));

// In the decl-specifier-seq, applying to a pointer type.
typedef int* IP;
_Nonnull IP d1 = nullptr;
IP _Nullable d2 = nullptr;
_Null_unspecified IP d3 = nullptr;
IP _Nullable_result d4 = nullptr;
using MP = int S::*;
_Nonnull MP d5 = &S::m;

// Function pointers, type-ids, casts and template arguments.
void (* _Nonnull fp1)(int) = nullptr;
using F = void (* _Nullable)(int);
auto c1 = static_cast<int* _Nonnull>(nullptr);
template <class T> struct Box { T v; };
Box<int* _Nullable> b1;
static_assert(__is_same(decltype(b1.v), int*));
int* _Nonnull* pnew = new int* _Nonnull[2];

// Dependent types: checked when instantiated.
template <class T> struct W { T _Nonnull p; T* _Nullable q; };
W<int*> w1;
template <class T> void h(T* _Nonnull t);
void use(int* p) { h(p); }

// A conversion function's type.
struct C { operator int* _Nonnull(); };
