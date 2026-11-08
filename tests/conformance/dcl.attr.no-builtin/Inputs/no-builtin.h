// Shared by the -fno-builtin IR tests (WP-808).
extern "C" void* memset(void*, int, unsigned long);
extern "C" void* memcpy(void*, const void*, unsigned long);
extern "C" double ceil(double);
extern "C" void ext(char*);
namespace std {
template <class T> struct remove_reference { using type = T; };
template <class T> struct remove_reference<T&> { using type = T; };
template <class T> constexpr typename remove_reference<T>::type&& move(T&& t) noexcept {
  return static_cast<typename remove_reference<T>::type&&>(t);
}
}
void* lowered(char* p, const char* q) {
  memcpy(p, q, 4);
  ext(p);
  __builtin_memcpy(p, q, 2);
  return memset(p, 0, 8);
}
double rounded(double x) { return ceil(x); }
int moved(int& x) {
  int&& r = std::move(x);
  return r;
}
__attribute__((no_builtin("memcpy"))) void named(char* p) { memcpy(p, p + 1, 1); }
struct B {
  B();
  ~B();
  virtual void v();
};
void use(B* b, void (*fp)(char*), char* p) {
  B local;
  b->v();
  fp(p);
  named(p);
}
int g = (ext(nullptr), 1);
struct V { virtual ~V(); };
struct W { virtual void w(); };
struct X : V, W { void w() override; };
void X::w() {}
void thrower();
void caller() noexcept { thrower(); }
