// [cpp.import], [cpp.module], spec/modules.md §3: a module or import
// directive ends at its `;`; declarations after it on the same line are
// ordinary text (clang accepts them; GCC 16 rejects them, "expected end
// of line"). kiln used to discard the rest of the line, dropping `g` and
// so `main` (WP-1198).
//@ expect: run
//@ std: c++20
//@ module: same_line=Inputs/same-line.cppm
//@ exit: 7
import same_line; int g() { return h(); }

int main() { return g() + k() + 2; }
