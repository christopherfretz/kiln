// [over.match.class.deduct]/1.8: a pack expansion element of the aggregate
// deduction candidate that is not the last element corresponds to no
// initializer clause (its pack is deduced empty); a trailing one may take
// fewer clauses than an earlier deduction of its pack has elements, which
// it must then agree with. A clause past the elements leaves no candidate.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 39:3 error "no viable constructor or deduction guide for deduction of template arguments of 'F'"
//@ diag: 31:30 note "candidate template ignored: deduced packs of different lengths for parameter 'T' (<X, Y, Z> vs. <X, W>)"
//@ diag: 31:30 note "implicit deduction guide declared as 'template <class ...T> F(L<T...>, T ...) -> F<T...>'"
//@ diag: 31:30 note "candidate function template not viable: requires 1 argument, but 3 were provided"
//@ diag: 31:30 note "implicit deduction guide declared as 'template <class ...T> F(F<T...>) -> F<T...>'"
//@ diag: 31:30 note "candidate function template not viable: requires 0 arguments, but 3 were provided"
//@ diag: 31:30 note "implicit deduction guide declared as 'template <class ...T> F() -> F<T...>'"
//@ diag: 40:3 error "no viable constructor or deduction guide for deduction of template arguments of 'C'"
//@ diag: 22:39 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 22:39 note "implicit deduction guide declared as 'template <class U, class ...T> C(C<U, T...>) -> C<U, T...>'"
//@ diag: 22:39 note "candidate function template not viable: requires 0 arguments, but 2 were provided"
//@ diag: 22:39 note "implicit deduction guide declared as 'template <class U, class ...T> C() -> C<U, T...>'"

template <class U, class... T> struct C : T... {
  U a;
  static constexpr int len = sizeof...(T);
};
C c1 = {[] { return 1; }};
static_assert(c1.len == 0);
C c2 = {1.5};

template <class... T> struct L {};
template <class... T> struct F : L<T...>, T... {};
struct X {};
struct Y {};
struct Z {};
struct W { operator Y(); };
F f1 = {L<X, Y, Z>{}, {}, {}};
F f2 = {L<X, Y, Z>{}, X{}, Y{}};
static_assert(sizeof(f2) == sizeof(F<X, Y, Z>));
F f3 = {L<X, Y, Z>{}, X{}, W{}};
C c3 = {1, 2};
