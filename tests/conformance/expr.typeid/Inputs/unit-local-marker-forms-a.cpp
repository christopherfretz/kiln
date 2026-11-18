// One of the two units of unit-local-marker-forms-run.cpp (the other is
// the same text with the other letter).
#include "unit-local-marker-forms.h"

static auto static_closure = [] {};
const auto const_closure = [] {};
namespace {
auto anon_closure = [] {};
struct I;
} // namespace
struct {
  int v;
} unnamed_a;

static void throw_closure() { throw static_closure; }
static bool catch_closure(void (*t)()) {
  try {
    t();
  } catch (decltype(static_closure)) {
    return true;
  } catch (...) {
  }
  return false;
}
static void throw_unnamed() { throw unnamed_a; }
static bool catch_unnamed(void (*t)()) {
  try {
    t();
  } catch (decltype(unnamed_a)) {
    return true;
  } catch (...) {
  }
  return false;
}

extern const Forms forms_a = {&typeid(static_closure), &typeid(const_closure),
                               &typeid(anon_closure),   &typeid(unnamed_a),
                               &typeid(I *),            throw_closure,
                               catch_closure,           throw_unnamed,
                               catch_unnamed};
