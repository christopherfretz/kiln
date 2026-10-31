// [temp.deduct]/8: an expression made invalid by substitution in a return type.
template <class T> constexpr auto neg(T t) -> decltype(-t) { return -t; }
constexpr int neg(...) { return 0; }
struct S {};
static_assert(neg(3) == -3);
static_assert(neg(S{}) == 0);
