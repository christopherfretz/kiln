// [dcl.stc] in C++03: `auto` followed by a type-specifier is the
// storage class (automatic storage duration), for a variable and for a
// block-scope anonymous union alike.
//@ expect: run
//@ std: c++98
//@ args: -pedantic-errors
//@ strict-diags
// C++98: `auto` before a type-specifier is the storage class, so these
// are automatic variables (clang 22 and GCC 16.1 agree).
struct Pair {
  int i;
};

int sum() {
  auto int a = 1;
  auto unsigned b = 2u;
  auto struct Local { int v; } c = { 3 };
  auto union {
    int whole;
    char part;
  };
  whole = 4;
  return a + static_cast<int>(b) + c.v + whole;
}

int main() { return sum() == 10 ? 0 : 1; }
