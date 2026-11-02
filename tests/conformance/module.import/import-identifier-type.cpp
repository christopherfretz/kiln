// [cpp.pre]/1: `import *p;` and `module *q;` are text lines (the token after
// `import`/`module` is `*`), so a class named `import` or `module` can
// still declare pointers at the start of a line.
//@ std: c++20
//@ strict-diags
//@ skip(clang): clang 19/22 treat any line-initial `import` as an import directive ("expected a module name after 'import'")
struct import {};
import *p;
import &r = *p;
struct module {};
module *q;
