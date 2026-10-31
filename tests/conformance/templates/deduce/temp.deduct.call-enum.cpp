// [temp.deduct.call]: an enumeration argument deduces the enumeration type.
enum class E : short { a = 3 };
template <class T> constexpr int sz(T) { return sizeof(T); }
static_assert(sz(E::a) == sizeof(short));
