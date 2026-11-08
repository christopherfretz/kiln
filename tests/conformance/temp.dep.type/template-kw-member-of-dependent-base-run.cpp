// [temp.dep.type]/6, [temp.names]/5: `Value::template convert<U>(k)` with `Value` the current
// instantiation and `convert` a member template of its dependent base names a member of an
// unknown specialization, looked up at instantiation (oneTBB's associative container tests).
//@ expect: run
//@ stdout: 1 0
#include <cstdio>
template <class K> struct Base {
  template <class U> static U convert(const K& v) { return U(v) + 1; }
};
template <class C> struct Value : Base<typename C::key_type> {
  template <class U> static bool compare(const typename C::key_type& k, U val) {
    return Value::template convert<U>(k) == val;
  }
};
struct Cont { using key_type = int; };
int main() { std::printf("%d %d\n", Value<Cont>::compare(4, 5L), Value<Cont>::compare(4, 4L)); }
