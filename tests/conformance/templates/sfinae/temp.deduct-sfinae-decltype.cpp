// [temp.deduct]/8: an invalid expression in decltype is a substitution failure.
struct S { constexpr int size() const { return 3; } };
template <class T> constexpr auto len(const T& t) -> decltype(t.size()) { return t.size(); }
constexpr int len(...) { return -1; }
static_assert(len(S{}) == 3);
static_assert(len(5) == -1);
