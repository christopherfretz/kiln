// One of two units of unit-local-type-info-libstdcxx-run.cpp: the same
// source in each but for the variable's name, so every unit-local type
// below has a twin of the same name in the other unit.
#include "unit-local-type-info.h"
namespace {
struct S : Base {};
template <class T> struct W {};
} // namespace
static const std::type_info &fn_local() {
  struct L {};
  return typeid(L);
}
static const std::type_info &lambda() {
  auto l = [] {};
  return typeid(l);
}
static S s_obj;
static Base *make() { return new S; }
static void throw_local() { throw S(); }
static void throw_local_ptr() { throw &s_obj; }
static void throw_shared() { throw Shared(); }
static int catch_local(void (*f)()) {
  try { f(); } catch (S &) { return 1; } catch (...) { return 0; }
  return 2;
}
static int catch_local_ptr(void (*f)()) {
  try { f(); } catch (S *) { return 1; } catch (...) { return 0; }
  return 2;
}
static int catch_shared(void (*f)()) {
  try { f(); } catch (Shared &) { return 1; } catch (...) { return 0; }
  return 2;
}
static int cast_local(Base *b) { return dynamic_cast<S *>(b) != nullptr; }
extern const Unit unit_b = {typeid(S), typeid(S *), typeid(S[2]), typeid(void(S)), fn_local(),
                          lambda(), typeid(W<S>), typeid(Shared), make, throw_local,
                          throw_local_ptr, throw_shared, catch_local, catch_local_ptr,
                          catch_shared, cast_local};
