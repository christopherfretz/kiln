export module imgsat;
export namespace sat {
template <class T> struct Box {
  static_assert(sizeof(T) > 1, "Box needs a wide type");
  static_assert(sizeof(T) < 64);
  T v;
  int get() const { return (int)v; }
};
template <class T> struct Outer {
  struct In {
    static_assert(sizeof(T) != 3);
    T w;
  };
};
inline int use() { Box<long> b{7}; return b.get(); }
}
