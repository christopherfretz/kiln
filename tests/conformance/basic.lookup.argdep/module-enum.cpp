// [basic.lookup.argdep]/3.3, /4: ADL finds an exported function of a
// named module that is not visible here (E is imported only by F) when it
// shares its innermost enclosing non-inline namespace P with an associated
// entity attached to E: an enumeration (clang 22 and 19 consider classes
// only and reject h with "use of undeclared identifier 'h'"; kiln follows
// the standard at every compat level, spec/modules.md §10 deviation 10),
// or a class, for k declared in the inline namespace P::I.
//@ expect: run
//@ std: c++20
//@ module: E=Inputs/adl-e.cppm
//@ module: F=Inputs/adl-f.cppm
//@ skip(clang): clang rejects h(get_en()) with "use of undeclared identifier 'h'" (spec/modules.md §10 deviation 10)
//@ exit: 3
import F;
int main() { return h(get_en()) + k(get_cl()); }
