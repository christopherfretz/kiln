// Debug info (WP-814): a class template specialization with type and value parameters.
//@ args: -g
template <typename T, int N> struct Box {
  T v[N];
  T first() const { return v[0]; }
};
int use() {
  Box<int, 2> b{{1, 2}};
  return b.first();
}
