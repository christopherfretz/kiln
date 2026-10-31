// [temp.func.order]: partial ordering between member function templates.
struct S {
  template <class T> constexpr int f(T) const { return 1; }
  template <class T> constexpr int f(T*) const { return 2; }
};
int i;
static_assert(S{}.f(1) == 1);
static_assert(S{}.f(&i) == 2);
