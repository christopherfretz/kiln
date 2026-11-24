// [dcl.array]/1, [dcl.fct]/5 across a module: instances made in the
// interface whose array bounds were substituted (and keep their
// substituted written forms, spec/types.md 7.2 "Written bounds") import
// with their values, and the importer's own instances of the same
// templates agree.
//@ module: subst_written_mod=Inputs/subst_written_mod.cppm
//@ std: c++26
//@ expect: run
//@ stdout: 16 9 9
import subst_written_mod;
extern "C" int printf(const char *, ...);

int main() {
  TB<n::S> tb{};
  printf("%d %d %d\n", tb_size(), tf_use(), tf(n::S(), nullptr));
  return sizeof(tb.dep) == 8 ? 0 : 1;
}
