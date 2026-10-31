// [temp.expl.spec]/1 (member of a class template): an explicit
// specialization of a member function or static data member of an
// implicit specialization.
template <class T> struct Num {
  static constexpr int id() { return 0; }
  static const int scale;
};
template <class T> const int Num<T>::scale = 1;
template <> constexpr int Num<long>::id() { return 8; }
template <> const int Num<long>::scale = 100;
static_assert(Num<int>::id() == 0);
static_assert(Num<long>::id() == 8);
static_assert(Num<int>::scale == 1);
static_assert(Num<long>::scale == 100);
