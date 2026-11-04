// [basic.stc.thread]/1, [expr.prim.id.unqual]/3, [class.copy.elision]/3:
// a block-scope `thread_local` variable has thread storage duration, so
// `return v;` naming it is not an implicitly movable entity: the
// id-expression stays an lvalue and binds the function's `T&` return,
// with or without `static`. Its address and a `T&` argument name the same
// thread-local object across calls; a static member function and a class
// type behave alike. The globals are `internal thread_local` (as clang's
// `@_ZZ1gvE1i`).
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 | 1 1 | 7 2
extern "C" int printf(const char*, ...);

int& g() { thread_local int i = 0; return i; }
int& h() { static thread_local int i = 2; return i; }
int* p() { thread_local int i = 0; return &i; }
int& paren() { thread_local int i = 6; return (i); }

void bump(int& r) { ++r; }
int pass() { thread_local int i = 0; bump(i); return i; }

template <class T> struct Box { T items[4]; int n = 0; };
inline Box<int*>& f() noexcept { thread_local Box<int*> v; return v; }
struct X {
  static Box<int>& b() noexcept { thread_local Box<int> s; return s; }
};
template <class T> T& tmpl() { thread_local T t{}; return t; }

int main() {
  ++g();
  int a = g();
  ++h();
  int c = h();
  ++*p();
  int d = *p() + 3;
  printf("%d %d %d %d | ", a, a + 1, c, d);
  f().n += 1;
  X::b().n = f().n;
  printf("%d %d | ", f().n, X::b().n);
  int e = ++paren();
  pass();
  tmpl<int>() = 2;
  printf("%d %d\n", e, pass() - 1 + tmpl<int>() - 1);
}
