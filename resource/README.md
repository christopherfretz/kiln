# resource/

Data kiln needs to look like clang 19 to the code it compiles (see
`docs/ARCHITECTURE.md` principle 3 and `spec/pp.md`). Everything here except
`include/LICENSE.TXT` and the READMEs is produced by `scripts/gen-resource.sh`
from the clang 19 oracle in the container. The script is idempotent: rerun it
after a clang update and review the diff. It refuses to run if `clang-19` is
not clang 19.

```
scripts/gen-resource.sh                      # uses clang-19 / clang++-19 on PATH
CLANG=... CLANGXX=... scripts/gen-resource.sh
```

## include/ — builtin headers

A filtered, unmodified copy of clang 19.1.7's resource-dir headers. kiln's
`-resource-dir` points here; it takes the place of
`/usr/lib/llvm-19/lib/clang/19/include` in the search order. Provenance, the
license, and the skip list are in `include/README.md`.

## predefines/ — predefined macros

One file per (target, standard): `<triple>.<std>.txt`, for triples
`aarch64-unknown-linux-gnu` (the container's native target) and
`x86_64-unknown-linux-gnu`, and std `c++17`, `c++20`, `c++23`, `c++26`. Each
is the output of

```
clang-19 --target=<triple> -std=<std> -E -dM -x c++ /dev/null | LC_ALL=C sort
```

so every line is `#define NAME VALUE` (the VALUE may be empty, e.g.
`__USER_LABEL_PREFIX__`). Only object-like macros appear. A file can be
pre-included verbatim.

`predefines/structure.diff` (also generated) summarizes how the sets relate:
std-to-std diffs per target, target-to-target diffs per std, strict-vs-GNU
dialect (`c++26` vs `gnu++26`), the version-encoding macros, and a check that
the location and time builtins are absent.

What to know when consuming these:

- **Std spelling:** clang 19 accepts both `-std=c++26` and `-std=c++2c`; the
  output is byte-identical. `__cplusplus` is `202400L` for C++26 (not final).
- **Dialect:** the files are the *strict* dialect (`-std=c++NN`, which sets
  `__STRICT_ANSI__`). clang's default is `gnu++17`. GNU mode drops
  `__STRICT_ANSI__` and adds `linux`, `unix`, `__GLIBCXX_TYPE_INT_N_0 __int128`,
  `__GLIBCXX_BITSIZE_INT_N_0 128`. Nothing else differs.
- **Shape of the std diffs:** std-to-std differences are confined to
  `__cplusplus`, `__cpp_*`, and char8_t atomics
  (`__{CLANG,GCC}_ATOMIC_CHAR8_T_LOCK_FREE`, from C++20). They are identical
  across both targets.
- **Shape of the target diffs:** arch macros (`__aarch64__`, `__ARM_*` vs
  `__x86_64__`, `__SSE*__`, `__MMX__`, `__k8*`), `long double` (binary128 on
  aarch64 vs x87 80-bit on x86_64), `wchar_t` signedness, `char` signedness
  (`__CHAR_UNSIGNED__` on aarch64), `__BITINT_MAXWIDTH__` (128 vs 8388608),
  `__FLOAT128__`/`__SIZEOF_FLOAT128__` (x86_64 only),
  `__GCC_HAVE_SYNC_COMPARE_AND_SWAP_16` (aarch64 only), `__seg_fs/__seg_gs`. The
  target diff is identical for every std.
- **Feature-test macros clang 19 sets in every mode, as extensions:**
  `__cpp_pack_indexing 202311L`, `__cpp_deleted_function 202403L`,
  `__cpp_placeholder_variables 202306L`, `__cpp_static_call_operator 202207L`,
  `__cpp_structured_bindings 202403L`, `__cpp_named_character_escapes`, and so on.
  These appear even in the C++17 files. `__cpp_concepts` is `202002` with no `L`
  suffix (verbatim from clang).
- **Version and vendor identity** (what kiln claims to be):
  `__clang__ 1`, `__clang_major__ 19`, `__clang_minor__ 1`,
  `__clang_patchlevel__ 7`, `__clang_version__ "19.1.7 (3+b1)"`,
  `__VERSION__ "Debian Clang 19.1.7 (3+b1)"` (the `Debian` and `(3+b1)` parts
  are the distro's vendor string), `__llvm__ 1`, `__GNUC__ 4`,
  `__GNUC_MINOR__ 2`, `__GNUC_PATCHLEVEL__ 1`, `__GNUG__ 4`,
  `__GXX_ABI_VERSION 1002`.
- **Time and location builtins:** `__DATE__`, `__TIME__`, `__TIMESTAMP__`,
  `__FILE__`, `__LINE__`, `__COUNTER__`, `__INCLUDE_LEVEL__`, `__BASE_FILE__`,
  `__FILE_NAME__` are *not* in `-dM` output (confirmed). The preprocessor must
  supply them.
- **Driver-default dependent:** the files reflect clang's defaults on this
  container, so some macros depend on command-line options. Measured on
  clang 19:
  `-O1+` removes `__NO_INLINE__` and adds `__OPTIMIZE__` (plus
  `__OPTIMIZE_SIZE__` for `-Os/-Oz`).
  `-fno-exceptions` removes `__EXCEPTIONS` and `__cpp_exceptions`.
  `-fno-rtti` removes `__GXX_RTTI` and `__cpp_rtti`.
  `-fno-pie` removes `__PIC__/__PIE__/__pic__/__pie__` (Debian defaults to PIE level 2).
  `-fPIC` removes `__PIE__/__pie__`.
  `-ffast-math` adds `__FAST_MATH__`, `__NO_MATH_ERRNO__`, `__ARM_FP_FAST`, and sets
  `__FINITE_MATH_ONLY__ 1`.
  `-ffreestanding` sets `__STDC_HOSTED__ 0`.
  `-pthread` adds `_REENTRANT`.
  `-fno-threadsafe-statics`, `-fno-sized-deallocation` and `-fno-char8_t` remove
  their `__cpp_*` macros.
  `-march`/`-mcpu` change the `__ARM_FEATURE_*` and `__SSE*__`/`__AVX*__` sets.
  `__GCC_HAVE_DWARF2_CFI_ASM 1` is present in the files, but the driver injects it
  as a `-D` to cc1 (it follows `-funwind-tables`/`-fasynchronous-unwind-tables`)
  rather than cc1 predefining it.

## include-paths/ — system include search order

`include-paths/<triple>.txt` (default stdlib, which is libstdc++ on Debian)
and `include-paths/<triple>.libc++.txt` (`-stdlib=libc++`): the
`#include "..."` / `#include <...>` blocks from
`clang++-19 --target=<triple> -stdlib=<lib> -E -v -x c++ /dev/null`, verbatim.
Lines starting with a space are search directories, in order. clang's
`ignoring nonexistent directory` lines are kept as `#` comments. These paths
are specific to this container and are a reference for the driver, not
something kiln hardcodes.

- **libstdc++ (aarch64):** `.../include/c++/14`, `.../include/aarch64-linux-gnu/c++/14`,
  `.../include/c++/14/backward`, then resource dir, `/usr/local/include`,
  `/usr/include/aarch64-linux-gnu`, `/usr/include`.
- **libc++:** the three libstdc++ dirs become a single
  `/usr/lib/llvm-19/bin/../include/c++/v1` (Debian keeps `__config_site` in
  the same directory, so there is no separate per-target `c++/v1` dir). The rest
  of the list is unchanged.
- **x86_64:** the container has no x86_64 GCC installation
  (`/usr/lib/gcc/x86_64-linux-gnu` is absent), so clang finds no libstdc++ and
  the default list starts at the resource dir. With `-stdlib=libc++` it uses the
  same (target-independent) libc++ headers as aarch64.
  `/usr/include/x86_64-linux-gnu` exists but holds only `asm/`, so there is no
  usable x86_64 glibc sysroot here. Actually compiling for x86_64 would need a
  cross sysroot (`libc6-dev-amd64-cross` or similar).
- Each file also records how the driver hands the dirs to `cc1`:
  `-internal-isystem` (C++ stdlib, resource dir, `/usr/local/include`) versus
  `-internal-externc-isystem` (`/usr/include/<multiarch>`, `/usr/include`).
  Headers in the second group are treated as implicitly `extern "C"` system
  headers. `clang -E -v` does not show this distinction, but kiln needs it.
- The quote (`"..."`) list is empty because there are no `-iquote` or `-I`
  flags. The resource dir always comes after the C++ stdlib dirs and before
  `/usr/local/include`.
