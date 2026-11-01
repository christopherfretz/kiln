// [temp.mem.class]/1, [basic.lookup.qual]/1: a member class of a member class of a class
// template, defined out of line, is named through the specialization (`O<int>::M::N`); the
// enclosing member class is instantiated to look `N` up
//@ expect: run
//@ strict-diags
template <class T>
struct O {
  struct M {
    struct N;
  };
};
template <class T>
struct O<T>::M::N {
  T v = 3;
};
int main() {
  O<int>::M::N n;
  return n.v == 3 ? 0 : 1;
}
