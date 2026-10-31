// [expr.sizeof]/5: sizeof... of a template and a function parameter pack.
template <class... Ts> constexpr unsigned long count(const Ts &...ts) { return sizeof...(Ts) + sizeof...(ts); }
static_assert(count(1, 2.0, 'c') == 6);
static_assert(count() == 0);
