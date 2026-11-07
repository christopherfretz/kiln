// [temp.deduct.conv]/4-5: for a reference target `U const &` the
// referee keeps its cv when the conversion template's result is a
// reference, so `operator T const &()` deduces `T = U` and binds the
// reference directly to the returned lvalue ([dcl.init.ref]/5.1.2).
// A non-reference result drops the cv of both. With a deleted
// `operator T()` next to it the lvalue result is the only candidate
// of [over.match.ref] (not an ambiguous conversion to a temporary).
//@ expect: run
//@ strict-diags
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
template <class A, class B> struct same { static const bool value = false; };
template <class A> struct same<A, A> { static const bool value = true; };

double g = 1.5;

template <class Source>
struct NoConvert {
  template <class Dest, class = typename en<same<Source, Dest>::value>::type>
  operator Dest () = delete;
  template <class Dest, class = typename en<same<Source, Dest>::value>::type>
  operator Dest const & () { return g; }
};

struct Both {
  template <class D> operator D () { return D(2); }
  template <class D> operator D const & () { return g; }
};

struct ValueOnly {
  template <class D> operator D () { return D(3); }
};

struct RefOnly {
  template <class D> operator D & () { return g; }
};

const double *take(double const &d) { return &d; }

struct yes {};
struct no {};
template <class U>
auto probe(int) -> decltype(take(NoConvert<U>()), yes());
template <class>
no probe(...);

int main() {
  if (take(NoConvert<double>()) != &g)
    return 1;
  if (take(Both()) != &g)
    return 2;
  const double &v = ValueOnly();
  if (v != 3.0 || &v == &g)
    return 3;
  const double &r = RefOnly();
  if (&r != &g)
    return 4;
  if (!same<decltype(probe<double>(0)), yes>::value)
    return 5;
  if (!same<decltype(probe<int>(0)), no>::value)
    return 6;
  return 0;
}
