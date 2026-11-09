// C11 6.5.1.1p3 / [expr.const] (_Generic in C++, a clang extension): a
// selection is a constant expression when its selected association is;
// an unselected one may be non-constant or a throw, and is never
// evaluated by the constant evaluator. The result keeps the selected
// expression's value category (decltype), an lvalue result is
// modifiable in a constexpr function, and selections in templates
// (dependent operand, type operand, NTTP) resolve at instantiation.
//@ expect: compile-ok
//@ strict-diags
int nonconst();
constexpr int a = _Generic(1, int: 1, default: nonconst());
static_assert(a == 1);
static_assert(_Generic(1, int: 1, default: 0));
static_assert(_Generic('c', int: 1, default: 0) == 0);
constexpr int f(int n) { int v = 3; _Generic(n, int: v) += n; return v; }
static_assert(f(4) == 7);
template <class T> constexpr int g(T t) { return _Generic(t, int: t * 2, long: 3, default: nonconst()); }
static_assert(g(5) == 10 && g(5L) == 3);
constexpr int arr[3] = {1, 2, 3};
static_assert(_Generic(arr, const int *: arr[2], default: 0) == 3);
template <class T> struct W { static constexpr int v = _Generic(T, int: 1, float: 2, default: 0); };
static_assert(W<int>::v == 1 && W<float>::v == 2 && W<char>::v == 0);
static_assert(__is_same(decltype(_Generic(1, int: arr[0])), const int &));
static_assert(__is_same(decltype(_Generic(1, int: 1)), int));
int y;
static_assert(__is_same(decltype(_Generic(1, int: static_cast<int&&>(y))), int &&));
constexpr int h = _Generic(1, int: 1, default: throw 0);
consteval int ce(int x) { return x; }
static_assert(_Generic(1, int: ce(3)) == 3);
template <int N> constexpr int nt = _Generic(N, int: N + 1);
static_assert(nt<4> == 5);
