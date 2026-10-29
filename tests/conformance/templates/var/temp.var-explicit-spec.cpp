// [temp.expl.spec]/1: an explicit specialization of a variable template
// replaces the primary's definition for those arguments.
template <class T> constexpr int rank = 0;
template <> constexpr int rank<char> = 1;
template <> constexpr int rank<short> = 2;
template <> constexpr int rank<int> = 3;
static_assert(rank<char> == 1);
static_assert(rank<short> == 2);
static_assert(rank<int> == 3);
static_assert(rank<long> == 0);
template <class T> constexpr const char *name = "?";
template <> constexpr const char *name<int> = "int";
static_assert(name<int>[0] == 'i' && name<long>[0] == '?');
