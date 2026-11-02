// [class.union.anon], [class.dtor]/13, [expr.const]: destroying an object
// whose class has an anonymous union member does not call a destructor of
// the anonymous union itself, which would be deleted when a variant member
// has a non-trivial destructor ([class.dtor]/7); the enclosing class's
// user-provided destructor destroys the active member explicitly. This is
// libc++ `__optional_destruct_base` (std::optional of a class with a
// constexpr destructor) in constant evaluation and at run time.
//@ expect: run
//@ stdout: 2
extern "C" int printf(const char*, ...);
int dtors = 0;
struct NT {
  int* count;
  constexpr ~NT() { ++*count; }
};
template <class T> struct opt {
  union {
    char null_;
    T val_;
  };
  bool engaged_;
  constexpr opt() : null_(), engaged_(false) {}
  constexpr opt(T v) : val_(v), engaged_(true) {}
  constexpr ~opt() {
    if (engaged_) val_.~T();
  }
};
struct Plain {
  union {
    NT a;
    long b;
  };
  constexpr Plain(int* c) : a{c} {}
  constexpr ~Plain() { a.~NT(); }
};
constexpr int count() {
  int n = 0;
  { opt<NT> empty; }
  { opt<NT> full(NT{&n}); }
  { Plain p(&n); }
  return n;
}
static_assert(count() == 3);  // with the temporary NT{&n}

int main() {
  {
    opt<NT> full(NT{&dtors});
    dtors = 0;
  }
  { Plain p(&dtors); }
  printf("%d\n", dtors);
}
