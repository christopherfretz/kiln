// [temp.deduct]/5: a default template argument supplies a non-deduced parameter.
template <class T, class R = long> constexpr R widen(T x) { return x; }
static_assert(sizeof(widen(1)) == sizeof(long));
static_assert(sizeof(widen<int, short>(1)) == sizeof(short));
