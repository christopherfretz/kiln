// [class.access.base]/5, [class.protected]/1: a protected member function
// template of a private base, named with a qualified template-id through
// an object of the derived class from a friend of that class, is
// accessible (as a member of the derived class it is private, and the
// friend may name private members). The template's specializations are
// non-static members, so the object's class is what [class.protected]
// checks. Reduced from protobuf's repeated_ptr_field.h.
//@ expect: run
template <class T> class It;
class Base {
 protected:
  template <class H> int Add(int v) { return v + 1; }
  int plain(int v) { return v + 2; }
};
template <class T> class F final : private Base {
  template <class U> friend class It;
  friend struct NT;
};
template <class T> struct It {
  F<T>* f;
  int go() { return f->Base::template Add<T>(3) + f->Base::plain(4); }
};
struct NT {
  F<int>* f;
  int go() { return f->Base::Add<int>(3) + f->Base::plain(4); }
};
int main() {
  F<int> x;
  It<int> i{&x};
  NT n{&x};
  return i.go() + n.go() == 20 ? 0 : 1;
}
