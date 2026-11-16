// [temp.deduct.general]/8: access checking is part of the substitution
// into a function template's signature, done in the context of the
// template, so a private member named there is a substitution failure
// even for a call made by a friend of the class: the other overload is
// chosen, the template is never accepted. A member template of the
// class sees its private names. Found running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
template<class T> char sf(typename T::type*);
template<class T> long sf(...);
struct R;
struct C {
  friend struct R;
private:
  typedef int type;
};
struct R {
  static const int v = sizeof(sf<C>(0));
};
static_assert(R::v == sizeof(long), "");

struct E {
private:
  typedef int type;
public:
  template<class T> static char mf(typename T::type*);
  template<class T> static long mf(...);
};
static_assert(sizeof(E::mf<E>(0)) == 1, "");

int main() { return R::v == sizeof(long) ? 0 : 1; }
