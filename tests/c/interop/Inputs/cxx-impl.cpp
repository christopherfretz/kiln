// The C++ side of c-calls-cxx.c: C language linkage on C++ definitions.
#include <string>
namespace {
struct Doubler {
  int operator()(int x) const { return x * 2; }
};
const std::string lang = "c++";
} // namespace
extern "C" int cxx_twice(int x) { return Doubler{}(x); }
extern "C" const char *cxx_lang() { return lang.c_str(); }
