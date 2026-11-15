// [temp.spec.partial.member]/2: the member partial specialization
// `inner<C, S>` of `outer<bool C>` is chosen for `outer<true>::inner<true,
// false>` at run time too (found running GCC's C++ testsuite).
//@ expect: run
//@ stdout: 1 0
extern "C" int printf(const char *, ...);
template <bool C>
struct outer {
  template <bool W, bool S>
  struct inner {
    static bool test() { return false; }
  };
  template <bool S>
  struct inner<C, S> {
    static bool test() { return true; }
  };
};
int main() {
  printf("%d %d\n", outer<true>::inner<true, false>::test(),
         outer<true>::inner<false, false>::test());
}
