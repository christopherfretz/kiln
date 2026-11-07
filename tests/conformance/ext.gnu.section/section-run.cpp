//@ expect: run
//@ strict-diags
// GCC "Common Variable Attributes"/"Common Function Attributes", section:
// the variable or function is placed in the named section (clang
// `SectionAttr`), so the ELF linker's `__start_NAME`/`__stop_NAME`
// symbols bound it. A registry collected this way (each translation unit
// adding an entry to one section) is how protobuf finds its weak default
// instances.
struct Entry { const char* name; int value; };

Entry first_entry __attribute__((section("kiln_reg"))) = {"first", 1};
[[gnu::section("kiln_reg")]] Entry second_entry = {"second", 2};

extern "C" {
extern const Entry __start_kiln_reg;
extern const Entry __stop_kiln_reg;
}

int counter() {
  static int calls __attribute__((section("kiln_counter"))) = 40;
  return ++calls;
}

__attribute__((section("kiln_text"))) int in_text(int x) { return x * 2; }

struct S {
  static int member __attribute__((section("kiln_member")));
  int method() __attribute__((section("kiln_text")));
};
int S::member = 5;
int S::method() { return member; }

template <class T> __attribute__((section("kiln_text"))) T twice(T t) { return t + t; }

int main() {
  const Entry* b = &__start_kiln_reg;
  const Entry* e = &__stop_kiln_reg;
  if (e - b != 2) return 1;
  int sum = 0;
  for (const Entry* p = b; p != e; ++p) sum += p->value;
  if (sum != 3) return 2;
  if (counter() != 41 || counter() != 42) return 3;
  if (in_text(4) != 8) return 4;
  if (S().method() != 5) return 5;
  if (twice(21) != 42) return 6;
  return 0;
}
