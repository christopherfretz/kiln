// [temp.arg.explicit]/9, [temp.deduct.call]/1: an explicitly specified
// pack element (`mk<int>()`) starts the pack, deduction from the call
// arguments extends it, and the defaulted parameter after the pack still
// takes its default. Named from a member of a class template, the
// specialization keeps exactly those arguments (WP-629).
//@ expect: run
//@ stdout: 1 2 3 2
template <class T> struct H {
  template <class... Args, class E = int>
  static int mk(Args... pairs) {
    static_assert(__is_same(E, int));
    return sizeof...(Args);
  }
  int e1() { return mk<int>(1); }
  int e2() { return mk<int>(1, 2); }
  int e3() { return mk<int, char>(1, 'a', 2L); }
  int e4() { return H::mk<T>(T(), T()); }
};
extern "C" int printf(const char*, ...);
int main() {
  H<long> h;
  printf("%d %d %d %d\n", h.e1(), h.e2(), h.e3(), h.e4());
}
