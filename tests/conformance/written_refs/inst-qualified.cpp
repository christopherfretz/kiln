// Per-specialization entries (spec/ast.md §18.9): dependent qualified
// names ([temp.dep.expr]/3, [basic.lookup.qual]) are looked up in the
// class the qualifier names once it is substituted: `T::value`, a
// dependent component `T::inner::` (the component gets its own `spec=`
// line naming the member class), an overloaded static member call
// `T::make(x)` resolved by overload resolution, and `Box<T>::limit`.
//@ written-refs: inst-qualified.refs
struct A {
  static const int value = 1;
  struct inner {
    static const int value = 2;
  };
  static int make(int) { return 3; }
  static int make(double) { return 4; }
};

template <class T> struct Box {
  static const int limit = 5;
};

template <class T> int read() {
  return T::value + T::inner::value + T::make(1.0) + Box<T>::limit;
}

int run() { return read<A>(); }
