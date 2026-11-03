// [temp.constr.atomic]: a non-comparison atom is shown as written; clang
// 22 then explains a false type trait (__is_trivially_copyable or
// std::is_trivially_copyable_v) as it does for a static_assert.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 23:15 error "static assertion failed"
//@ diag: 23:15 note "because 'NT' does not satisfy 'Trivial'"
//@ diag: 21:38 note "because '__is_trivially_copyable(NT)' evaluated to false"
//@ diag: 21:38 note "'NT' is not trivially copyable"
//@ diag: 21:38 note "because it has a user provided copy constructor"
//@ diag: 22:8 note "'NT' defined here"
//@ diag: 26:19 error "no matching function for call to 'g'"
//@ diag: 25:30 note "candidate template ignored: constraints not satisfied [with T = NT]"
//@ diag: 25:11 note "because 'NT' does not satisfy 'StdTrivial'"
//@ diag: 24:41 note "because 'std::is_trivially_copyable_v<NT>' evaluated to false"
//@ diag: 24:41 note "'NT' is not trivially copyable"
//@ diag: 24:41 note "because it has a user provided copy constructor"
//@ diag: 22:8 note "'NT' defined here"
namespace std { template<class T> constexpr bool is_trivially_copyable_v = __is_trivially_copyable(T); }
template <class T> concept Trivial = __is_trivially_copyable(T);
struct NT { NT(const NT &); };
static_assert(Trivial<NT>);
template <class T> concept StdTrivial = std::is_trivially_copyable_v<T>;
template <StdTrivial T> void g(T);
void use(NT &n) { g(n); }
