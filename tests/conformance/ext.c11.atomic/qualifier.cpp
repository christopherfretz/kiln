// Clang extension (C11 6.7.3 in C++): the bare _Atomic type qualifier names
// _Atomic of the unqualified type with the cv-qualifiers outside it; ++/--
// yield the value type, compound assignment yields the _Atomic lvalue.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 24:34 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 25:35 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 27:36 warning "expression with side effects has no effect in an unevaluated context"
_Atomic int a;
int _Atomic b;
const _Atomic int c = 1;
typedef const int CI;
_Atomic CI d = 2;
volatile _Atomic(int) *vp;
_Atomic int arr[2];
_Atomic _Atomic(int) twice;
static_assert(__is_same(decltype(a), _Atomic(int)));
static_assert(__is_same(decltype(b), _Atomic(int)));
static_assert(__is_same(decltype(c), const _Atomic(int)));
static_assert(__is_same(decltype(d), const _Atomic(int)));
static_assert(__is_same(decltype(arr), _Atomic(int)[2]));
static_assert(__is_same(decltype(twice), _Atomic(int)));
static_assert(sizeof(_Atomic int) == 4);
static_assert(__is_same(decltype(++a), int &));
static_assert(__is_same(decltype(a++), int));
static_assert(__is_same(decltype(a + 1), int));
static_assert(__is_same(decltype(a += 1), _Atomic(int) &));
void f() { _Atomic long x = 3; x += a; x = *vp; }
