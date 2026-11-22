// [cpp.replace]: one definition expanded several times. Every expansion
// names the same definition, and each `L` is body token 0 of it, spelled
// at one position (spec/pp.md "Preprocessing record"; the 500-invocation
// form is crates/kiln_pp/tests/pp_record.rs).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: one-definition.record
namespace L { void log(const char *); }
#define LOG(m) L::log(m)
void f() {
  LOG("a");
  LOG("b");
  LOG("c");
}
