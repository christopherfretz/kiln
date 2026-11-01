// [temp.friend]/1, [temp.param]/12: a class template first declared by a friend declaration in
// another class template (libc++ `__wrap_iter` befriends `basic_string` before <string> declares
// it) takes its parameters and default arguments from its namespace-scope declarations
//@ expect: run
//@ strict-diags
template <class I> class wi {
  I i;
  template <class C, class T, class A> friend class bs;
public:
  explicit wi(I x) : i(x) {}
};
template <class C> struct tr {};
template <class C> struct al {};
template <class C, class T = tr<C>, class A = al<C> > class bs;
typedef bs<char> str;
template <class C, class T, class A> class bs {
public:
  int n = 1;
  template <class It> bs(It, It) : n(2) {}
  bs() = default;
  int at(int) const;
  int get(wi<int> w) const { return w.i; }
};
template <class C, class T, class A> int bs<C, T, A>::at(int k) const { return n + k; }
static_assert(__is_same(str, bs<char, tr<char>, al<char> >));
extern template int bs<char>::at(int) const;
template int bs<char>::at(int) const;
template <class C, class T, class A> bs<C, T, A> operator+(const C*, const bs<C, T, A>& s) { return s; }
template str operator+<char, tr<char>, al<char> >(const char*, const str&);
int main() {
  const char* p = "ab";
  str s = "x" + str();
  str r(p, p + 2);
  return (s.at(1) == 2 && r.n == 2 && s.get(wi<int>(5)) == 5) ? 0 : 1;
}
