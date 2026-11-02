// [basic.lookup.argdep]/3.3, /4: ADL finds an exported function of a
// named module that is not visible here (E is imported only by F) when it
// shares its innermost enclosing non-inline namespace P with an associated
// entity attached to E: an enumeration (clang 22 considers classes only
// and rejects h, a deviation, spec/modules.md §5), or a class, for k
// declared in the inline namespace P::I.
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/adl-e.cppm %{test_dir}/Inputs/adl-f.cppm -fmodule-file=E=%{test_dir}/Inputs/adl-e.cppm -fmodule-file=F=%{test_dir}/Inputs/adl-f.cppm
//@ skip(clang): clang needs a two-step --precompile build of the Inputs modules that the harness cannot drive
//@ exit: 3
import F;
int main() { return h(get_en()) + k(get_cl()); }
