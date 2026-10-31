// [meta.unary.prop]/9 (clang's __is_constructible / __is_nothrow_constructible):
// whether `T t(declval<Args>()...);` is well-formed, checked as an
// unevaluated operand with access from the translation unit. libc++'s
// is_constructible and is_nothrow_constructible use the builtins directly.
//@ expect: compile-ok
//@ strict-diags
struct A {};
struct B { B(int); };
struct C { explicit C(int, double) noexcept; C(const C&) = delete; };
class P { P(); public: P(int) noexcept(false); };
struct Abs { virtual void f() = 0; };
struct D { D(B); };
struct Agg { int x; double y; };
union U { int i; float f; };
enum class E { a };

static_assert(__is_constructible(int));
static_assert(__is_constructible(int, int));
static_assert(__is_constructible(int, double));
static_assert(!__is_constructible(int, int, int));
static_assert(!__is_constructible(int, int*));
static_assert(__is_constructible(A));
static_assert(__is_constructible(A, const A&));
static_assert(__is_constructible(A, A&&));
static_assert(!__is_constructible(A, int));
static_assert(!__is_constructible(B));
static_assert(__is_constructible(B, int));
static_assert(__is_constructible(B, long));
static_assert(__is_constructible(C, int, double));
static_assert(!__is_constructible(C, int));
static_assert(!__is_constructible(C, const C&));
static_assert(!__is_constructible(P));          // private
static_assert(__is_constructible(P, int));
static_assert(!__is_constructible(Abs));
static_assert(__is_constructible(D, B));
static_assert(__is_constructible(D, int));      // one user conversion per argument
static_assert(__is_constructible(Agg, int, double));  // [dcl.init.general]/16.6.2.2
static_assert(!__is_constructible(Agg, int, A));
static_assert(__is_constructible(U));
static_assert(__is_constructible(E));
static_assert(!__is_constructible(E, int));
static_assert(__is_constructible(int[3]));
static_assert(!__is_constructible(int[]));
static_assert(!__is_constructible(void));
static_assert(!__is_constructible(const void));
static_assert(!__is_constructible(int, void));
static_assert(!__is_constructible(void()));
static_assert(!__is_constructible(int&));
static_assert(__is_constructible(int&, int&));
static_assert(!__is_constructible(int&, int));
static_assert(!__is_constructible(int&, int&&));
static_assert(__is_constructible(const int&, int));
static_assert(__is_constructible(int&&, int));
static_assert(!__is_constructible(int&&, int&));
static_assert(__is_constructible(const A&, A));
static_assert(!__is_constructible(A&, int, int));
static_assert(__is_constructible(int*, decltype(nullptr)));
static_assert(__is_constructible(void (*)(), void()));
static_assert(__is_constructible(void (&)(), void()));

static_assert(__is_nothrow_constructible(int));
static_assert(__is_nothrow_constructible(int, double));
static_assert(__is_nothrow_constructible(A, const A&));
static_assert(!__is_nothrow_constructible(B, int));
static_assert(__is_nothrow_constructible(C, int, double));
static_assert(!__is_nothrow_constructible(P, int));
static_assert(!__is_nothrow_constructible(int, int*));

// Dependent operands and packs are evaluated at substitution.
template <class T, class... Args>
constexpr bool ctor = __is_constructible(T, Args...);
template <class T, class... Args>
constexpr bool nothrow = __is_nothrow_constructible(T, Args...);
static_assert(ctor<A> && ctor<B, int> && !ctor<B> && ctor<C, int, double>);
static_assert(!ctor<C, int, double, int> && !ctor<int&>);
static_assert(nothrow<int, int> && !nothrow<B, int>);

template <class T, class U>
struct is_ctor { static constexpr bool value = __is_constructible(T, const U&); };
static_assert(is_ctor<A, A>::value && !is_ctor<C, C>::value);

template <class T> struct W { W(T); };
static_assert(__is_constructible(W<int>, int) && !__is_constructible(W<int>, A));

// A reference bound to a temporary of an implicit conversion still counts
// (clang 19; only __reference_constructs_from_temporary tells them apart).
struct Q { operator A() const; };
static_assert(__is_constructible(const int&, long) && __is_constructible(int&&, long));
static_assert(__is_constructible(const A&, Q));
