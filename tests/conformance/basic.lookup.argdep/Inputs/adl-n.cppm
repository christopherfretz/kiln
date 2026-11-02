// Translation unit #2 of the [basic.lookup.argdep]/5 example.
export module N;
import M;
export R::X make();
namespace R { static int g(X); }
export template<typename T, typename U> void apply(T t, U u) {
  f(t, u);
  g(t);
}
