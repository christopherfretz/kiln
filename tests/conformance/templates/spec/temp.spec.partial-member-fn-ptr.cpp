// [temp.deduct.type]/8: pointer-to-member-function types, with their
// cv-qualifiers, deduced in partial specializations (the
// <type_traits> member-function-pointer traits).
template <class T> struct mfp { static constexpr int arity = -1; };
template <class R, class C> struct mfp<R (C::*)()> {
  static constexpr int arity = 0;
  static constexpr bool is_const = false;
};
template <class R, class C, class A> struct mfp<R (C::*)(A)> {
  static constexpr int arity = 1;
  static constexpr unsigned long arg = sizeof(A);
};
template <class R, class C> struct mfp<R (C::*)() const> {
  static constexpr int arity = 0;
  static constexpr bool is_const = true;
};
struct S;
static_assert(mfp<int>::arity == -1);
static_assert(mfp<int (S::*)()>::arity == 0);
static_assert(!mfp<int (S::*)()>::is_const);
static_assert(mfp<void (S::*)() const>::is_const);
static_assert(mfp<void (S::*)(char)>::arity == 1);
static_assert(mfp<void (S::*)(double)>::arg == 8);
