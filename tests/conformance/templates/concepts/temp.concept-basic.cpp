//@ expect: compile-ok
// [temp.concept]/5: a concept-id is a prvalue of type bool, true iff its
// normalized constraint expression is satisfied.
template <class T> concept Big = sizeof(T) > 4;
static_assert(Big<double>);
static_assert(!Big<char>);
constexpr bool b = Big<long long>;
static_assert(b);
