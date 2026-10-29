// [temp.mem]/1: member templates of a non-template class.
struct S {
  template <class U> struct In { U u; static constexpr int n = sizeof(U); };
  template <class U> static constexpr int size = sizeof(U);
  template <class U> using ptr = U *;
};
static_assert(S::In<long>::n == 8);
static_assert(S::size<short> == 2);
S::ptr<int> p = nullptr;
S::In<char> c = {'x'};
static_assert(sizeof(c) == 1);
