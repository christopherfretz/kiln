// [expr.const]/5, [dcl.pre]/10 (static_assert), [temp.arg.nontype]/2:
// clang's `__builtin_classify_type(e)` is an integer constant expression
// whatever `e` is: GCC's type class of `e`'s type (clang
// `EvaluateBuiltinClassifyType`, the same in clang 19 and 22), with the
// operand never evaluated. In C++ clang uses only some of GCC's classes:
// void 0; integer 1 (every character and integer type: GCC's char class 2
// is never used, and a reference names its referee, so GCC's reference
// class 6 is never used either); enum 3 (scoped too); bool 4; pointer 5
// (an array, a string literal and a function classify as the decayed
// pointer, so GCC's function 10, array 14 and string 15 are never used);
// offset (pointer to data member) 7; real 8; complex 9; record 12 (also
// pointers to member functions: GCC's method class 11 is never used);
// union 13; vector 19; and no_type_class -1 for std::nullptr_t (GCC's lang
// class 16 is never used). bit-int 18: builtin-classify-type-bitint.cpp.
// spec/sema.md §5.4 (WP-419).
//@ expect: compile-ok
//@ strict-diags
#define CT(e, n) static_assert(__builtin_classify_type(e) == (n), #e)
enum E { e0 };
enum class SE : bool { a };
struct S { int m; void f(); int bf : 3; };
union U { int i; float f; };
struct NT { NT(); ~NT(); };
typedef int v4 __attribute__((vector_size(16)));
typedef float ev4 __attribute__((ext_vector_type(4)));
void fn();
int i, arr[3];
int &ri = i;
_Atomic(int) ai;
extern const char c;
// void (no_type_class is not used for void).
CT(fn(), 0);
CT((void)0, 0);
// integer, char.
CT(i, 1);
CT(c, 1);
CT('a', 1);
CT(u8'a', 1);
CT(L'a', 1);
CT(u'a', 1);
CT(U'a', 1);
CT((signed char)0, 1);
CT((unsigned short)0, 1);
CT(0ull, 1);
CT((__int128)0, 1);
CT(S().bf, 1);
CT(ai, 1);
// reference: an expression has its referee's type.
CT(ri, 1);
// enum, bool.
CT(e0, 3);
CT(SE::a, 3);
CT(true, 4);
CT(i == 0, 4);
// pointer, array, string, function.
CT(&i, 5);
CT((void *)0, 5);
CT(arr, 5);
CT("abc", 5);
CT(L"abc", 5);
CT(fn, 5);
CT(&fn, 5);
// offset (pointer to data member) and method (pointer to member function).
CT(&S::m, 7);
CT(&S::f, 12);
// real, complex.
CT(1.0f, 8);
CT(1.0, 8);
CT(1.0L, 8);
CT(__builtin_complex(1.0, 2.0), 9);
// record, union, lambda closure.
CT(S(), 12);
CT(NT(), 12);
CT([] {}, 12);
CT(U(), 13);
// vector.
CT(v4(), 19);
CT(ev4(), 19);
// nullptr (no_type_class).
CT(nullptr, -1);
// The operand is unevaluated: a call of a non-constexpr function, a side
// effect and an odr-use of a non-constant are all fine.
int g();
CT(g(), 1);
CT(i++, 1);
CT(i = 2, 1);
// A non-type template argument, an array bound, a case label.
template <int N> struct X { static constexpr int value = N; };
static_assert(X<__builtin_classify_type(arr)>::value == 5);
int bound[__builtin_classify_type(1.0)];
static_assert(sizeof(bound) == 8 * sizeof(int));
constexpr int sw(double d) {
  switch (__builtin_classify_type(d)) {
  case __builtin_classify_type(0.5f): return 1;
  default: return 0;
  }
}
static_assert(sw(0) == 1);
// Dependent operands are classified per instantiation.
template <class T> constexpr int ct(T t) { return __builtin_classify_type(t); }
static_assert(ct(1) == 1 && ct('a') == 1 && ct(nullptr) == -1 && ct(1.5) == 8);
static_assert(ct(arr) == 5 && ct(S()) == 12 && ct(U()) == 13 && ct(&S::m) == 7);
template <class T> constexpr int cv = __builtin_classify_type(T());
static_assert(cv<int *> == 5 && cv<E> == 3 && cv<bool> == 4 && cv<_Complex float> == 9);
static_assert(__has_builtin(__builtin_classify_type));
static_assert(__has_constexpr_builtin(__builtin_classify_type));
