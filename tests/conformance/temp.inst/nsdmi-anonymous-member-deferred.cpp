// [temp.inst]/3: the implicit instantiation of a class template
// specialization does not instantiate the default member initializers of
// its members, including those of the fields of an anonymous struct or
// union member ([class.union.anon]). They are instantiated when a
// constructor or aggregate initialization uses them. libc++ 22's
// `_LIBCPP_COMPRESSED_PAIR(pointer, __cap_ = nullptr, ...)` is an anonymous
// struct; `sizeof(vector<char, A>)` with a fancy pointer that does not
// convert from `nullptr` must not instantiate it
// (`containers/sequences/vector/abi.compile.pass.cpp`).
//@ expect: run
struct NoNull {
  short s;
};

template <class P>
struct V {
  struct {
    P a = nullptr;
    int b = 3;
  };
  union {
    P c = nullptr;
    int d;
  };
};
static_assert(sizeof(V<NoNull>) == 3 * sizeof(int));

template <class P>
struct W {
  struct {
    P a = nullptr;
    int b = 3;
  };
  int e = b + 1;
};

template <class T>
struct C {
  struct {
    T x = T(5);
  };
  constexpr C() {}
};
static_assert(C<int>().x == 5);

int main() {
  W<int*> w;
  V<int*> v;
  return (w.a == nullptr && w.b == 3 && w.e == 4 && v.c == nullptr && v.b == 3) ? 0 : 1;
}
