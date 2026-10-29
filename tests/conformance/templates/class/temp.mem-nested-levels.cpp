// [temp.mem]/1, [temp.inst]/1: a member template of a member template;
// each level substitutes its own parameters.
template <class T> struct L1 {
  template <class U> struct L2 {
    template <class V> struct L3 {
      static constexpr int value = sizeof(T) * 100 + sizeof(U) * 10 + sizeof(V);
    };
  };
};
static_assert(L1<char>::L2<short>::L3<int>::value == 124);
static_assert(L1<long>::L2<char>::L3<char>::value == 811);
