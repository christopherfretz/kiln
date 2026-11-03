// WP-432: a call through a function reference that folds to the function
// is a call of that function (clang's `CalleePtr`), so its `nounwind`
// lets the caller be inferred `nounwind` too.

int f(int x) { return x + 1; }
int (&r)(int) = f;
constexpr int (&cr)(int) = f;
static_assert(&cr == &f);
int (*p)(int) = r;
int (*q)(int) = cr;
int call() { return r(3) + cr(4); }
struct S { int (&m)(int); };
constexpr S s{f};
static_assert(&s.m == f);
int (&rr)(int) = s.m;
int call2() { return s.m(5) + rr(6); }
// A pointer variable is not folded: the call stays a `call ptr %…`.
int call3() { return p(7); }
