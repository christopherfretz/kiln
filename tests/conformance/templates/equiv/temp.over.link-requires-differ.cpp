// [temp.over.link]/7: templates whose trailing requires-clauses are not
// equivalent are distinct overloads.
template <class T> constexpr int k(T) requires (sizeof(T) > 1) { return 1; }
template <class T> constexpr int k(T) requires (sizeof(T) == 1) { return 2; }
static_assert(k(1) == 1);
static_assert(k('c') == 2);
