// [dcl.fct.def.general]/8, [expr.const]: `__func__` is a static array of
// const char usable in constant expressions; the Oven reads the names
// (template suffixes included) and compares them with literals. libc++'s
// `__type_name` and LLVM's TypeID read `__PRETTY_FUNCTION__` this way.
//@ expect: compile-ok
//@ oven: steps<=20000 allocs<=0
constexpr bool eq(const char* a, const char* b) {
  while (*a && *a == *b) {
    ++a;
    ++b;
  }
  return *a == *b;
}
constexpr unsigned len(const char* s) {
  unsigned n = 0;
  while (s[n]) ++n;
  return n;
}

template <class T>
constexpr const char* type_name() { return __PRETTY_FUNCTION__; }

template <class T, int N>
struct Box {
  constexpr const char* who() const { return __PRETTY_FUNCTION__; }
  static constexpr const char* fn() { return __func__; }
};

constexpr const char* plain() { return __FUNCTION__; }

static_assert(eq(type_name<int>(), "const char *type_name() [T = int]"));
static_assert(eq(type_name<Box<char, 2>>(),
                 "const char *type_name() [T = Box<char, 2>]"));
static_assert(eq(Box<long, -1>().who(),
                 "const char *Box<long, -1>::who() const [T = long, N = -1]"));
static_assert(eq(Box<int, 0>::fn(), "fn"));
static_assert(eq(plain(), "plain"));
static_assert(len(type_name<unsigned>()) == 42);

constexpr auto lam = [] { return __func__; };
static_assert(eq(lam(), "operator()"));

int main() {
  static_assert(sizeof(__func__) == 5);
  static_assert(__func__[0] == 'm' && __func__[4] == '\0');
  static_assert(eq(__PRETTY_FUNCTION__, "int main()"));
}
