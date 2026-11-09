// gmf-spec-merge.cpp: a global module fragment that defines the same
// entities as its importer, which defines the first group (namespace p)
// before its `import` and the second (namespace q) after it.
module;
namespace p {
struct C { int x = 1; };
template <class T> struct L { static constexpr int s = 0; };
template <> struct L<float> { static constexpr int s = 1; };
template <class T> struct L<T *> { static constexpr int s = 2; };
}
namespace q {
template <class T> struct L { static constexpr int s = 0; };
template <> struct L<double> { static constexpr int s = 3; };
template <class T> struct L<T &> { static constexpr int s = 4; };
}
export module gsm;
export constexpr int gsm_get() {
  return p::C{}.x + p::L<float>::s + p::L<int *>::s + q::L<double>::s + q::L<int &>::s;
}
