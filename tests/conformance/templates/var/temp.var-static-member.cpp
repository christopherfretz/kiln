// [temp.mem]/1, [class.static.data]: a static data member template of a
// non-template class.
struct Traits {
  template <class T> static constexpr bool is_int = false;
  template <class T> static constexpr unsigned long size = sizeof(T);
};
template <> constexpr bool Traits::is_int<int> = true;
static_assert(Traits::is_int<int>);
static_assert(!Traits::is_int<long>);
static_assert(Traits::size<short> == 2);
