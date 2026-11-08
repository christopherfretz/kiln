// [dcl.ambig.res]/1, [temp.res.general]/3: `has_key<char, int>::type` names
// a type, found in the base `impl<int>::apply<char, int>` of the
// specialization, where `impl<int>` is an explicit specialization that
// defines the member template `apply` the primary only declares. So
// `int (*p)(has_key<char, int>::type)` is a declaration and the
// parenthesized `(int (*)(has_key<char, int>::type))g` is a cast.
// Boost.MPL `map.cpp:51` (`has_key_impl<map_tag>`) hits this.
//@ expect: run
//@ stdout: 6 4
extern "C" int printf(const char*, ...);
template<class T> struct tag_of { typedef int type; };
template<class Tag> struct impl { template<class S, class K> struct apply; };
template<> struct impl<int> {
  template<class S, class K> struct apply { typedef long type; };
};
template<class S, class K>
struct has_key : impl<typename tag_of<S>::type>::template apply<S, K> {};
int g(long i) { return int(i) * 2; }
int main() {
  int r = ((int (*)(has_key<char, int>::type))g)(3);
  int (*p)(has_key<char, int>::type) = g;
  printf("%d %d\n", r, p(2));
}
