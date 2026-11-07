// [namespace.udecl]/10: a using-declarator in a class is a redeclaration
// only when an earlier one names the member through the same class
// (clang CheckUsingDeclRedeclaration compares the nested-name-specifiers).
// `using TDB::Write; using PDB::Write;`, where PDB's own `using TDB::Write`
// brings the same base overload into PDB, is valid: the member already
// introduced is not introduced again (clang CheckUsingShadowDecl). A
// transactional DB class hierarchy declares its Write overloads this way.
//@ expect: run
//@ stdout: 30 31 31 7
extern "C" int printf(const char*, ...);
struct DB {
  virtual int Write(int) = 0;
  virtual int Write(int, long) { return 2; }
  int Write(char) { return 7; }
  virtual ~DB() = default;
};
struct TDB : DB { using DB::Write; int Write(int) override { return 10; } };
struct PDB : TDB { using TDB::Write; int Write(int) override { return 20; } };
struct WP : PDB {
  using TDB::Write;
  int Write(int) override { return 30; }
  using PDB::Write;
  int Write(int, long) override { return 31; }
};
int main() {
  WP w;
  DB& d = w;
  printf("%d %d %d %d\n", w.Write(1), w.Write(1, 2L), d.Write(1, 2L), w.Write('c'));
}
