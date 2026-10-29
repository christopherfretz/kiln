// [temp.spec.partial.member]/1: the members of a partial specialization
// are unrelated to the primary's; they are instantiated from the partial
// specialization.
template <class T> struct holder {
  T value;
  static constexpr int kind = 0;
};
template <class T> struct holder<T *> {
  T *first, *second;
  static constexpr int kind = 1;
  static constexpr unsigned long pointee_size() { return sizeof(T); }
};
static_assert(sizeof(holder<char>) == 1 && holder<char>::kind == 0);
static_assert(sizeof(holder<char *>) == 16 && holder<char *>::kind == 1);
static_assert(holder<long *>::pointee_size() == 8);
