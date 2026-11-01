// [temp.mem.class]/1: a member class of a class template may be defined outside the class
// template definition; the definition is instantiated with the enclosing specialization
// (libc++ <ostream>: `template <class C, class T> class basic_ostream<C, T>::sentry`)
//@ expect: run
//@ strict-diags
template <class C, class T>
class basic_ostream {
public:
  class sentry;
  int v = 3;
};
template <class C, class T>
class basic_ostream<C, T>::sentry {
  bool ok;

public:
  explicit sentry(basic_ostream<C, T>& os) : ok(os.v == 3) {}
  explicit operator bool() const { return ok; }
};
int main() {
  basic_ostream<char, int> os;
  basic_ostream<char, int>::sentry s(os);
  return bool(s) ? 0 : 1;
}
