// [temp.deduct.type]/10: a pack deduced from a function type's parameters.
template <class R, class... A> constexpr unsigned long arity(R (*)(A...)) { return sizeof...(A); }
constexpr int add(int a, int b) { return a + b; }
constexpr int none() { return 0; }
static_assert(arity(add) == 2);
static_assert(arity(none) == 0);
