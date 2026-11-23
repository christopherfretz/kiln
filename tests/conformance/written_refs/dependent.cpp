// The written-reference table (spec/ast.md §18): in a template pattern
// a non-dependent name has its entity and a dependent one has none
// (`(dependent)`): a call found by argument-dependent lookup at
// instantiation, a dependent member, a member of a dependent qualifier.
// Each name has one entry; instantiation adds `spec=` lines (the separate
// per-specialization table, §18.9) for `pattern<U>` and `Holder<U>`.
//@ written-refs: dependent.refs
int helper(int);
int fixed = 3;

template <class T> struct Holder {
  T value;
  int size() const { return value.size() + fixed; }
};

template <class T> int pattern(T t) {
  int a = helper(fixed);
  int b = convert(t);
  int c = t.count + T::base + Holder<T>{t}.size();
  return a + b + c + helper(sizeof(t));
}

namespace user {
struct U {
  int count;
  static const int base = 1;
  int size() const { return 1; }
};
int convert(U);
} // namespace user

int run() { return pattern(user::U{2}) + Holder<user::U>{}.size(); }
