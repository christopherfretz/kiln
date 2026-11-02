// [expr.rel]/1 (CWG583): the operands of a relational operator have
// arithmetic, enumeration or pointer type. `std::nullptr_t` is none of them,
// so `<` with a nullptr_t operand is ill-formed (and SFINAE-detectable, as
// libc++'s support.types/nullptr_t.pass.cpp checks), and a null pointer
// constant does not convert to the other operand's pointer type for an
// ordered comparison (clang: "ordered comparison between pointer and zero").
// `==` keeps working ([expr.eq]/4).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:12 error "invalid operands to binary expression"
//@ diag: 23:12 error "invalid operands to binary expression"
//@ diag: 24:18 error "invalid operands to binary expression"
//@ diag: 25:12 error "invalid operands to binary expression"
//@ diag: 26:12 error "ordered comparison between pointer and zero"
using nt = decltype(nullptr);
template <class T> T &&declval();
template <class T, class = void> struct has_less { static constexpr bool value = false; };
template <class T>
struct has_less<T, decltype((void)(declval<T>() < nullptr))> { static constexpr bool value = true; };
static_assert(!has_less<nt>::value && !has_less<int *>::value && !has_less<int>::value);
void f(int *p, int *q, nt n) {
  (void)(n < n);
  (void)(p >= n);
  (void)(nullptr > nullptr);
  (void)(n <= 0);
  (void)(0 > p);
  (void)(n == n && p != n && n == 0 && p != 0 && p < q);
}
