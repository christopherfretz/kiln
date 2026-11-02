// [over.match.viable]/3, [over.match.conv]/1, [class.conv.fct]: a
// conversion function whose trailing requires-clause is not satisfied
// is not viable, so an unconstrained-looking better match (the
// non-const overload for a non-const object) does not hide the
// satisfied one. The libc++ mdspan test type `IntConfig`
// (`containers/views/mdspan/ConvertibleToIntegral.h`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:5 error "no viable conversion from 'C<false, false, true, true>' to 'int'"
//@ diag: 17:13 note "candidate function not viable: constraints not satisfied"
//@ diag: 17:53 note "because 'false' evaluated to false"
//@ diag: 18:13 note "candidate function not viable: constraints not satisfied"
//@ diag: 18:58 note "because 'false' evaluated to false"
template <bool conv_c, bool conv_nc, bool nt_c, bool nt_nc> struct C {
  int val;
  constexpr explicit C(int v) : val(v) {}
  constexpr operator int() noexcept(nt_nc) requires(conv_nc) { return val; }
  constexpr operator int() const noexcept(nt_c) requires(conv_c) { return val + 1; }
};
using CT = C<true, false, true, false>;
static_assert(__is_convertible(CT, int));
static_assert(__is_nothrow_constructible(int, CT));
static_assert(__is_convertible(const CT&, int));
CT c(1);
static_assert(noexcept(static_cast<int>(c)));
constexpr int conv(int i) { return i; }
static_assert(conv(CT(1)) == 2);
static_assert(conv(C<true, true, true, true>(1)) == 1);
static_assert(!__is_convertible(C<false, false, true, true>, int));
static_assert(__is_convertible(C<false, true, false, false>, int));
static_assert(!__is_nothrow_constructible(int, C<false, true, false, false>));
int x = C<false, false, true, true>(0);
