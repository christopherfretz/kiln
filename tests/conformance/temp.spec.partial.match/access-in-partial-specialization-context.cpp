// [temp.spec.partial.match]/2, [temp.deduct.general]/8, [class.access.general]/7:
// the template arguments of a partial specialization are checked in the
// context of the partial specialization. One that is a member of the
// class granting access sees its private names (class and variable
// templates alike); a friend template of the class grants access to all
// its partial specializations; a friend of the class that only names the
// specialization gives nothing, for a variable template too. Found
// running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
template<class, class = void> struct H { static const int value = 0; };
template<class T> struct H<T, typename T::type> { static const int value = 1; };

struct D {
private:
  typedef void type;
public:
  template<class, class = void> struct M { static const int value = 0; };
  template<class T> struct M<T, typename T::type> { static const int value = 1; };
  template<class, class = void> static constexpr int v = 0;
  template<class T> static constexpr int v<T, typename T::type> = 1;
};
static_assert(D::M<D>::value == 1, "");
static_assert(D::v<D> == 1, "");

template<class, class = void> struct G { static const int value = 0; };
template<class T> struct G<T, typename T::type> { static const int value = 1; };
struct F {
  template<class, class> friend struct G;
private:
  typedef void type;
};
static_assert(G<F>::value == 1, "");

template<class, class = void> constexpr int vt = 0;
template<class T> constexpr int vt<T, typename T::type> = 1;
struct Q;
struct C {
  friend struct Q;
private:
  typedef void type;
};
struct Q {
  static constexpr int f() { return vt<C> + H<C>::value; }
};
static_assert(Q::f() == 0, "");

int main() { return Q::f(); }
