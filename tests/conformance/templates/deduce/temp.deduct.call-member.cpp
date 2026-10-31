// [temp.deduct.call]: calls of member function templates, implicit and explicit object, deduced and explicit arguments.
struct S {
  int k = 3;
  template <class T> constexpr int get(T t) const { return k + t; }
  template <class T> constexpr int get2() const { return get<T>(1) + get(2); }
  template <class T> static constexpr int sget(T t) { return t; }
  constexpr int use() const { return get(1) + sget(2) + get2<int>(); }
};
static_assert(S{}.use() == 4 + 2 + 9);
static_assert(S{}.get(1) == 4);
static_assert(S{}.get<long>(1) == 4);
static_assert(S::sget<int>(5) == 5);
template <class U> struct W {
  U u;
  template <class T> constexpr U add(T t) const { return u + t; }
  constexpr U run() const { return add(1) + add<int>(2); }
};
static_assert(W<int>{1}.run() == 5);
