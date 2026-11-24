// [temp.expl.spec]/1, [temp.arg.nontype]/2, [module.interface]: an
// explicit specialization exported with its template, whose non-type
// argument calls a constexpr function of the module (an integer, a bool
// or a pointer value), is the specialization the importer names. The
// importer never evaluates the argument again: a module image carries
// the converted value, and a unit whose argument's value it cannot hold
// (the pointer `g()`) is replayed (spec/modules.md 11.11 "Arguments as
// written"). Reduced from real code.
//@ module: call_argument_mod=Inputs/call_argument_mod.cppm
//@ std: c++26
//@ expect: run
//@ stdout: 2 0 5 1
import call_argument_mod;
extern "C" int printf(const char *, ...);

int main() {
  printf("%d %d %d %d\n", P<int, 2>::k, P<int, 3>::k, Q<g()>::k, BT<true>::k);
}
