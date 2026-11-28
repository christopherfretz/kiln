// [temp.deduct.general]/8 (WP-3697): a function template specialization
// whose signature substitution failed on an incomplete class is not
// permanently memoized as failed; once the class is completed (at
// namespace or block scope), a later use re-deduces and succeeds.
//@ expect: compile-ok
struct S;
template <class T> auto g(T* t) -> decltype(t->size());
long g(...);

long a(S* p) {
  static_assert(sizeof(g(p)) == sizeof(long));
  return g(p);
}

struct S {
  int size();
};

long b(S* p) {
  static_assert(sizeof(g(p)) == sizeof(int));
  return 0;
}

// Block-scope local class completed later.
void local_case() {
  struct L;
  L* p = nullptr;
  static_assert(sizeof(g(p)) == sizeof(long));
  struct L {
    int size();
  };
  static_assert(sizeof(g(p)) == sizeof(int));
}
