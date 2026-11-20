// [class.bit]/3: there are no pointers to bit-fields, so `&C::b` naming a
// bit-field (directly, through an anonymous member, or after template
// substitution) is ill-formed
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:20 error "address of bit-field requested"
//@ diag: 14:21 error "address of bit-field requested"
//@ diag: 15:40 error "address of bit-field requested"
//@ diag: 16:21 note "in instantiation of variable template specialization 'tp' requested here"
struct S { int bf : 3; int x; struct { int ab : 2; }; };
constexpr auto ok = &S::x;
static_assert(ok != nullptr);
constexpr auto p = &S::bf;
constexpr auto pa = &S::ab;
template <class T> constexpr auto tp = &T::bf;
constexpr auto tq = tp<S>;
