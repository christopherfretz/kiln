// [temp.deduct.type]/8: R(*)(A) deduces the return and parameter types.
int twice(int x) { return 2 * x; }
template <class R, class A> constexpr int arity1(R (*)(A)) { return sizeof(R) + sizeof(A); }
static_assert(arity1(twice) == 2 * sizeof(int));
