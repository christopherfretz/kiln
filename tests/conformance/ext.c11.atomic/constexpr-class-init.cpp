// Clang extension: an _Atomic(T) object of class type T initialized from a
// T (NonAtomicToAtomic over the T's construction) in a constant expression
// holds that T (libc++'s `__cxx_atomic_base_impl(T)` mem-initializer, the
// constexpr `std::atomic<T>` constructors; WP-334). The size-promoted
// _Atomic(S3) keeps its padding byte. The T's construction is evaluated:
// a failing one, or a non-constant source, is not constant.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:16 error "constexpr variable 'bad_ctor' must be initialized by a constant expression"
//@ diag: 16:46 note "division by zero"
//@ diag: 39:25 note "in call to 'P(0)'"
//@ diag: 41:17 error "constexpr variable 'bad_src' must be initialized by a constant expression"
//@ diag: 41:33 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 40:5 note "declared here"
struct S3 { char a, b, c; };
struct P { int x; constexpr P(int v) : x(100 / v) {} };
struct D : S3 { int extra; };

template <class T> struct B {
  constexpr B() = default;
  constexpr explicit B(T v) noexcept : a(v) {}
  _Atomic(T) a;
};

constexpr B<S3> s(S3{1, 2, 3});
static_assert(sizeof(s) == 4);
constexpr B<P> p(P(4));
constexpr B<S3> braced{S3{}};

constexpr int local() {
  B<P> q(P(5));
  _Atomic(S3) from_derived = D{{7, 8, 9}, 1};
  _Atomic(P) direct = P(20);
  (void)from_derived, (void)direct;
  return 1;
}
static_assert(local() == 1);

constexpr B<P> bad_ctor{P(0)};
int g;
constexpr B<S3> bad_src(S3{char(g), 0, 0});
