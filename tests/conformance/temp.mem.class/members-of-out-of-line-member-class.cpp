// [temp.mem.class]/1, [temp.mem.func]/1, [temp.static]/1: the member functions and static
// data members of an out-of-line member class are themselves defined out of line with the
// enclosing class template's head
//@ expect: run
//@ strict-diags
template <class T>
struct S {
  struct sentry;
};
int dtors = 0;
template <class T>
struct S<T>::sentry {
  T v;
  static T count;
  sentry(T x);
  ~sentry();
  T get() const;
};
template <class T>
S<T>::sentry::sentry(T x) : v(x) { ++count; }
template <class T>
S<T>::sentry::~sentry() { ++dtors; }
template <class T>
T S<T>::sentry::get() const { return v + count; }
template <class T>
T S<T>::sentry::count = 10;
int main() {
  int r;
  {
    S<int>::sentry s(5);
    r = s.get();
  }
  return (r == 16 ? 0 : 1) + (dtors == 1 ? 0 : 2);
}
