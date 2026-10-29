// [temp.deduce.type]/8: pointer-to-member types deduced in partial
// specializations.
template <class T> struct member_of { static constexpr bool value = false; };
template <class M, class C> struct member_of<M C::*> {
  static constexpr bool value = true;
  static constexpr unsigned long size = sizeof(M);
};
struct S { int i; double d; };
static_assert(!member_of<int>::value);
static_assert(member_of<int S::*>::value);
static_assert(member_of<double S::*>::size == 8);
static_assert(member_of<char S::*>::size == 1);
