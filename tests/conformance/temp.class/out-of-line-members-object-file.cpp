// [temp.class]/3, [temp.mem.func], [temp.static], [temp.mem]: members of a class
// template defined out of line (constructor, member function, operator, member
// template, static data member, nested-class member, `typename C<T>::X` return
// types), used by implicit and explicit instantiations, are emitted into the
// object file and link. Regression test for the WP-126 codegen panic
// (`irtype.rs` padding underflow on the dependent pattern), fixed by WP-142
// (codegen skips templated top-level declarations); re-checked in WP-933 with
// -c and -S -emit-llvm on aarch64 and x86_64.
//@ expect: run
//@ strict-diags
template <class T> struct Box {
  T v;
  struct Inner {
    T a, b;
    Inner sum(Inner o) const;
  };
  Box(T x);
  T get() const;
  template <class U> U as() const;
  Box operator+(const Box&) const;
  static int count;
  Inner pair() const;
  typename Box<T>::Inner pair2() const;
};
template <class T> Box<T>::Box(T x) : v(x) { ++count; }
template <class T> T Box<T>::get() const { return v; }
template <class T> template <class U> U Box<T>::as() const { return static_cast<U>(v); }
template <class T> Box<T> Box<T>::operator+(const Box& o) const { return Box(v + o.v); }
template <class T> int Box<T>::count = 0;
template <class T> typename Box<T>::Inner Box<T>::pair() const { return {v, v}; }
template <class T> typename Box<T>::Inner Box<T>::pair2() const { return {v, v + 1}; }
template <class T>
typename Box<T>::Inner Box<T>::Inner::sum(Inner o) const {
  return {a + o.a, b + o.b};
}
template struct Box<long>;

int main() {
  Box<int> b(3);
  Box<double> d(1.5);
  int s = (b + Box<int>(4)).get();
  Box<int>::Inner p = b.pair().sum(b.pair2());
  Box<long> l(5);
  if (s != 7 || d.as<double>() != 1.5 || b.as<long>() != 3) return 1;
  if (Box<int>::count != 3 || Box<double>::count != 1 || Box<long>::count != 1) return 2;
  if (p.a != 6 || p.b != 7 || l.pair2().b != 6) return 3;
  return 0;
}
