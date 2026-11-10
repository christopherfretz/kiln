// Interface unit of module `imgdtt` for image-default-temporaries-run.cpp:
// a template whose for-range-initializer calls a function with a default
// argument holding a temporary (module `imgdt`). The call is dependent, so
// the default argument is rebuilt, and its temporary extended to the loop
// (P2718R0), when the importer instantiates the pattern.
export module imgdtt;
import imgdt;
namespace dt {
export template <class U> void tloop(U u) {
  for (int x : mku(u))
    put(x + 10);
  put(0);
}
}
