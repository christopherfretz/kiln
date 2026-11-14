// Debugger: member functions declared through a function typedef
// (`static Fn f;`, `Fn g;`) get a DISubroutineType as their declaration's
// type, as clang's `castAs<FunctionProtoType>` gives it. kiln gave them the
// typedef itself, a DW_TAG_typedef, which LLVM's verifier rejects ("invalid
// subroutine type"): llc warned "ignoring invalid debug info" and dropped
// the whole module's debug information, and hod declined it as
// `debug:invalid`. Reduced from real code (a static member declared
// through another class's function typedef).
//@ expect: debugger
//@ gdb: break S::f
//@ gdb: break S::g
//@ gdb: run
//@ gdb: print x
//@ gdb: continue
//@ gdb: print x
//@ gdb: print *this
//@ gdb: ptype S
//@ gdb-check: Breakpoint 1, S::f (x=41)
//@ gdb-check: $1 = 41
//@ gdb-check: Breakpoint 2, S::g (this=0x
//@ gdb-check: $2 = 7
//@ gdb-check: $3 = {k = 3}
//@ gdb-check: type = struct S {
//@ gdb-check: int k;
//@ gdb-check: static int f(int);
//@ gdb-check: int g(int);
typedef int Fn(int);

struct S {
  int k;
  static Fn f;
  Fn g;
};

int S::f(int x) { return x + 1; }
int S::g(int x) { return x + k; }

int main() {
  S s{3};
  return S::f(41) + s.g(7) - 52;
}
