export module lclam;
// A lambda, and a generic lambda (a closure type with a member template;
// a local class itself has none, [temp.mem]/2), in a local class's member.
export inline int lclam_call(int x) {
  struct L {
    int a;
    int run() const {
      auto f = [this](int y) { return a + y; };
      return f(10);
    }
  };
  return L{x}.run();
}
export inline long lclam_generic(int x) {
  struct G {
    int a;
    long run() const {
      auto g = [this](auto t) { return a + t; };
      return g(2) + g(3L);
    }
  };
  return G{x}.run();
}
