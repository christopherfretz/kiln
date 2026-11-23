// Per-specialization entries (spec/ast.md §18.9.2): a default argument
// is instantiated where a call uses it ([temp.inst]/12) and a default
// member initializer where a constructor uses it ([temp.inst]/4). The
// written site has one entry; each specialization whose call or
// constructor ran it adds a `spec=` line: the function template
// specialization or the member function of the class template
// specialization whose parameter's default it is, the data member of the
// class template specialization for a default member initializer. Two
// calls of one specialization make one line.
//@ written-refs: inst-default-arg.refs
struct P {
  static int start() { return 1; }
};
struct Q {
  static int start() { return 2; }
};

template <class T> int from(int n = T::start()) { return n; }

template <class T> struct Holder {
  int first = T::start();
  int pick(int n = T::start()) const { return n + first; }
};

int run() {
  Holder<P> h;
  Holder<Q> k;
  return from<P>() + from<P>() + from<Q>() + h.pick() + k.pick();
}
