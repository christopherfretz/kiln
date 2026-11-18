// C++98 under -pedantic-errors: what is not a C++11 extension. An
// elaborated friend naming a specialization of a template template
// parameter, and `= 0` on a function declared through a typedef of
// function type (a pure-specifier, not a default member initializer);
// `-Wno-long-long` silences `long long` (clang 22, GCC 16.1).
//@ expect: compile-ok
//@ std: c++98
//@ args: -pedantic-errors -Wno-long-long
//@ strict-diags
template <template <typename> class Holder>
struct Grants {
  friend class Holder<int>;
};

typedef void handler_fn(int) const;

struct Base {
  virtual handler_fn handle = 0;
};

long long widest = 1LL;
