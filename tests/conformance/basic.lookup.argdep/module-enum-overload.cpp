// [basic.lookup.argdep]/3.3, /4, [over.match.best]: the argument's type
// P::Un is an enumeration attached to E2, which is reachable but not
// visible here (only F2 imports it). So ADL finds E2's exported P::h,
// which shares Un's innermost enclosing namespace. P::h(Un) is an exact
// match and beats the visible ::h(int), which needs a promotion, so the
// program exits 1. This is the standard's answer and kiln's at every
// compat level. clang 22 and 19 consider associated classes only, call
// ::h(int) and exit 7: a deliberate deviation (spec/modules.md §5,
// §10 deviation 10).
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/adl-e2.cppm %{test_dir}/Inputs/adl-f2.cppm -fmodule-file=E2=%{test_dir}/Inputs/adl-e2.cppm -fmodule-file=F2=%{test_dir}/Inputs/adl-f2.cppm
//@ skip(clang): clang needs a two-step --precompile build of the Inputs modules that the harness cannot drive; it also exits 7 (deviation 10)
//@ exit: 1
import F2;
int h(int) { return 7; }
int main() { return h(get_un()); }
