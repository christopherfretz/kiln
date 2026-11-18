// The three forms of unit-local-marker-forms-run.cpp, defined the same way
// in two units (unit-local-marker-forms-a.cpp, -b.cpp).
#include <typeinfo>

struct Forms {
  const std::type_info *static_closure, *const_closure, *anon_closure, *unnamed, *incomplete;
  void (*throw_closure)();
  bool (*catch_closure)(void (*)());
  void (*throw_unnamed)();
  bool (*catch_unnamed)(void (*)());
};

extern const Forms forms_a, forms_b;
