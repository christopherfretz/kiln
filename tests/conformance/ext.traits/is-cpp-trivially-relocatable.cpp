// __builtin_is_cpp_trivially_relocatable(T) (clang 21+; the
// [class.prop] "trivially relocatable" rules of P2786 without its
// keywords, WP-263): scalar types, arrays of trivially relocatable
// types, and classes eligible for trivial relocation (no virtual base,
// every base and non-reference member trivially relocatable, destructor
// not deleted) that are unions without user-declared special members or
// are default-movable (move construction and assignment select members
// neither user-provided nor deleted, destructor not user-provided).
// Oracle-checked by hand with clang++-22 (22.1.8): same answers.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:12 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 32:8 note "forward declaration of 'Inc'"
struct A { ~A(); };
struct B { ~B(); };
B::~B() = default;
struct C { ~C() = default; };
struct D { D(D &&) = default; D &operator=(D &&) = default; };
struct E { E(const E &); };
struct F { F(F &&); };
union U { int i; ~U() = default; };
union V { int i; };
union W { int i; W(const W &); };
struct G { int &r; };
struct H : virtual C {};
struct I { ~I() = delete; };
struct J { J(const J &) = default; };
struct K { K &operator=(K &&) = delete; };
struct L { E e; };
struct M : C { V v; int a[2]; };
struct Inc;
template <class T> constexpr bool r = __builtin_is_cpp_trivially_relocatable(T);

static_assert(r<int> && r<int *> && r<int C::*> && r<int[3]> && r<C[2]> && r<const C>);
static_assert(!r<int &> && !r<void> && !r<int()>);
static_assert(!r<A> && !r<B> && r<C> && r<D> && !r<E> && !r<F>);
static_assert(r<U> && r<V> && !r<W> && !r<G> && !r<H> && !r<I>);
static_assert(r<J> && !r<K> && !r<L> && r<M>);
bool bad = __builtin_is_cpp_trivially_relocatable(Inc);
