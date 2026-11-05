// [expr.call]/8, [expr.shift]/4, [expr.ass]/1, [expr.sub]/1, [expr.ref]
// (C++17, P0145R3): the stricter evaluation order of postfix, shift,
// assignment and subscript operands. WP-646 feature probe.
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ stdout: ab cd ef gh ij
extern "C" int printf(const char*, ...);
struct Out { const Out& operator<<(const char* s) const { printf("%s", s); return *this; } };
const char* say(const char* s) { printf("%s", s); return ""; }
struct F { void call(const char*) const {} };
const F& getf() { printf("a"); static F f; return f; }
int arr[2];
int& el(const char* s) { printf("%s", s); return arr[0]; }
int idx(const char* s) { printf("%s", s); return 0; }
int (*fp(const char* s))(const char*) { printf("%s", s); return [](const char*) { return 0; }; }
int main() {
  getf().call(say("b"));        // postfix object before arguments
  printf(" ");
  Out{} << say("c") << say("d"); // left to right
  printf(" ");
  el("f") = idx("e");            // right operand of = first
  printf(" ");
  arr[idx("g")] = 0, (void)0;
  printf("h ");
  fp("i")(say("j"));             // callee before arguments
  printf("\n");
}
