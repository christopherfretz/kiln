// [cpp.pre]/1, [lex.name]/2: `import` and `module` are directive-introducing
// only at the start of a line and followed by a name, header-name, `<`,
// string literal, `:` or (`module`) `;`; elsewhere they are ordinary
// identifiers, and a translation unit without directives is no module unit.
//@ std: c++20
//@ strict-diags
int import = 1;
int module = 2;
int f() {
  import = 3;
  module = 4;
  import
      = 5;
  return import + module;
}
struct S {
  int import, module;
};
int g(S s) { return s.import + s.module + f(); }
