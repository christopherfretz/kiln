// Shared by unit-local-type-info-libstdcxx-run.cpp and its two units.
#include <typeinfo>
struct Base { virtual ~Base(); };
struct Shared : Base {};
struct Unit {
  const std::type_info &local, &local_ptr, &local_array, &local_fn, &fn_local, &lambda, &tmpl, &shared;
  Base *(*make)();
  void (*throw_local)();
  void (*throw_local_ptr)();
  void (*throw_shared)();
  int (*catch_local)(void (*)());
  int (*catch_local_ptr)(void (*)());
  int (*catch_shared)(void (*)());
  int (*cast_local)(Base *);
};
extern const Unit unit_a, unit_b;
