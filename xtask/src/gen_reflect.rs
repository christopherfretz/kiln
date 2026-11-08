//! `cargo xtask gen-reflect`: writes the reflection perf shapes S1–S6 of
//! `docs/plans/m6-reflection.md` §3.8, and the reference-workload shapes S7
//! (a binding generator) and S8 (the bindings it prints, compiled) of
//! `docs/status/perf-reference-workload.md`, and S9 and S10
//! (reflection-keyed instantiations, the mangler's load, S10 on deep
//! template-ids: spec/reflection.md §8.3), as bench
//! entries under `bench/reflect/` (`spec/bench.md` "Reflection shapes").
//!
//! Each shape is a function of one size parameter N and nothing else: the
//! same N gives the same bytes on every host, so the committed files (at
//! [`Shape::default_n`]) are checked against the generator by a unit test and
//! `--scaling` (`xtask/src/bench/scaling.rs`) can build any N. The sources
//! are written from the IS `<meta>` interface (GCC 16.1's spelling) and from
//! the plan's shape descriptions alone.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// One shape.
pub struct Shape {
    /// `s1` … `s10`: the file stem under `bench/reflect/`.
    pub id: &'static str,
    /// The N the committed file is generated at (small enough for the
    /// merge-queue S-tier, `docs/plans/m6-reflection.md` §3.9).
    pub default_n: usize,
    /// N must be a multiple of this.
    pub step: usize,
    /// What N counts, and what the shape exercises.
    pub what: &'static str,
    /// Where the shape is specified: `docs/plans/m6-reflection.md` §3.8
    /// (S1–S6), else the document that added it (the header's second line).
    source: &'static str,
    /// The WP that owns the shape's reflection family (the `xfail(kiln)`
    /// reason of an entry that does not compile yet), or, for a shape added
    /// after §3.8, the WP that added it; and the other families it needs.
    pub owner: &'static str,
    pub needs: &'static str,
    /// The entry compiles today against GCC 16.1's libstdc++ (the GCC
    /// oracle's `--gcc-toolchain`, which kiln is given in the sweeps and in
    /// bench runs): it carries no `xfail(kiln)`. S1–S6 were generated
    /// `xfail(kiln)` against kiln's own headers until their families
    /// landed and WP-638 moved them here; S7 and S8 were written for it.
    pub libstdcxx: bool,
    /// The entry compiles today without any library: no `#include
    /// <meta>`, no `xfail(kiln)`, and a `mode:` that runs codegen.
    pub bare: bool,
    body: fn(usize, &mut String),
    /// The `oven:` budget, a function of N.
    budget: fn(usize) -> String,
}

/// The section that specifies S1–S6.
const PLAN_SECTION: &str = "docs/plans/m6-reflection.md §3.8";

/// The shapes, in order.
pub const SHAPES: [Shape; 10] = [
    Shape {
        id: "s1",
        default_n: 100,
        step: 1,
        what: "N aggregates x 16 members; a serializer over nonstatic_data_members_of + \
               identifier_of + member splices in `template for`",
        source: PLAN_SECTION,
        owner: "WP-478",
        needs: "WP-474 (member splices), WP-475 (identifier_of), WP-479 (define_static_array)",
        libstdcxx: true,
        bare: false,
        body: s1,
        budget: |_| "peak_arena<=65536".into(),
    },
    Shape {
        id: "s2",
        default_n: 50,
        step: 1,
        what: "N enums x 64 enumerators; enum<->string tables via enumerators_of",
        source: PLAN_SECTION,
        owner: "WP-478",
        needs: "WP-474 (expression splices), WP-475 (identifier_of), WP-479 (define_static_array)",
        libstdcxx: true,
        bare: false,
        body: s2,
        budget: |_| "peak_arena<=65536".into(),
    },
    Shape {
        id: "s3",
        default_n: 100,
        step: 1,
        what: "define_aggregate: N struct-of-arrays twins of 4-member structs, plus one \
               struct of 10*N members",
        source: PLAN_SECTION,
        owner: "WP-480",
        needs: "WP-473 (consteval blocks), WP-482 (substitute), WP-478 (members)",
        libstdcxx: true,
        bare: false,
        body: s3,
        budget: |n| format!("peak_bytes<={}", 64 * 10 * n + 65536),
    },
    Shape {
        id: "s4",
        default_n: 100,
        step: 1,
        what: "S1's aggregates with annotations; the serializer keeps the members \
               annotations_of_with_type selects",
        source: PLAN_SECTION,
        owner: "WP-481",
        needs: "WP-478 (annotations_of_with_type), WP-474, WP-475, WP-479",
        libstdcxx: true,
        bare: false,
        body: s4,
        budget: |_| "peak_arena<=65536".into(),
    },
    Shape {
        id: "s5",
        default_n: 1000,
        step: 100,
        what: "N substitute + template_arguments_of + static_data_members_of round trips \
               (N distinct instantiations), 100 per evaluation",
        source: PLAN_SECTION,
        owner: "WP-482",
        needs: "WP-478 (static_data_members_of), WP-479 (reflect_constant, extract)",
        libstdcxx: true,
        bare: false,
        body: s5,
        budget: |_| "peak_arena<=65536".into(),
    },
    Shape {
        id: "s6",
        default_n: 500,
        step: 1,
        what: "a namespace of N declarations scanned by members_of, 100 times",
        source: PLAN_SECTION,
        owner: "WP-478",
        needs: "WP-472 (namespace reflections)",
        libstdcxx: true,
        bare: false,
        body: s6,
        budget: |n| format!("peak_bytes<={}", 16 * n + 65536),
    },
    Shape {
        id: "s7",
        default_n: 40,
        step: 1,
        what: "a binding generator: N annotated classes walked by members_of + \
               annotations_of_with_type, each binding built as a std::string and printed \
               from a template for that splices the bound members",
        source: "docs/status/perf-reference-workload.md",
        owner: "WP-551",
        needs: "a libstdc++ with <meta> (GCC 16.1)",
        libstdcxx: true,
        bare: false,
        body: s7,
        budget: |n| format!("peak_bytes<={}", 64 * n + 65536),
    },
    Shape {
        id: "s8",
        default_n: 40,
        step: 1,
        what: "the bindings S7 prints, compiled: N class_<T> instantiations of a \
               nanobind-style binding library (casters, call thunks, constexpr signatures)",
        source: "docs/status/perf-reference-workload.md",
        owner: "WP-551",
        needs: "a libstdc++ with <meta> (GCC 16.1)",
        libstdcxx: true,
        bare: false,
        body: s8,
        budget: |_| "peak_arena<=65536".into(),
    },
    Shape {
        id: "s9",
        default_n: 500,
        step: 1,
        what: "N reflection-keyed instantiations: f<^^Ti>() and S<^^Ti>::count for N types, \
               and f<^^W<W<W<Ti>>>>() for every fourth (the reflection mangler's load)",
        source: "spec/bench.md \"Reflection shapes\"",
        owner: "WP-501",
        needs: "nothing beyond `^^`",
        libstdcxx: false,
        bare: true,
        body: s9,
        budget: |_| "peak_arena<=65536".into(),
    },
    Shape {
        id: "s10",
        default_n: 200,
        step: 1,
        what: "N reflection-keyed instantiations f<^^X>() and S<^^X>::count, each X a distinct \
               template-id six levels deep with six arguments per level (the reflection \
               mangler's load on long nested types)",
        source: "spec/bench.md \"Reflection shapes\"",
        owner: "WP-879",
        needs: "nothing beyond `^^`",
        libstdcxx: false,
        bare: true,
        body: s10,
        budget: |_| "peak_arena<=65536".into(),
    },
];

/// The shape named `id` (`s1` … `s10`, case-insensitive).
pub fn shape(id: &str) -> Option<&'static Shape> {
    SHAPES.iter().find(|s| s.id.eq_ignore_ascii_case(id))
}

/// kiln's extra flags for `shape`: for a [`Shape::libstdcxx`] shape, the
/// `--gcc-toolchain=` of the GCC oracle (`KILN_GCC`, else `g++-16`, as
/// `spec/harness.md` "GCC oracle"), whose libstdc++ has `<meta>`. Empty for
/// the other shapes, and when no GCC oracle runs (the compile then fails on
/// the missing header, which the report shows).
pub fn kiln_flags(shape: &Shape) -> Vec<String> {
    if !shape.libstdcxx {
        return Vec::new();
    }
    let (gcc, _) = crate::oracle::gcc_from(std::env::var(crate::oracle::GCC_ENV).ok().as_deref());
    crate::oracle::gcc_toolchain_of(&gcc)
        .map(|p| vec![format!("--gcc-toolchain={}", p.display())])
        .unwrap_or_default()
}

/// The bench entry for `shape` at `n`.
pub fn generate(shape: &Shape, n: usize) -> String {
    let mut out = String::new();
    let upper = shape.id.to_uppercase();
    let source = shape.source;
    // A §3.8 shape names the families it exercises (still needs, while it
    // is `xfail(kiln)`); a later one the WP that added it.
    let needs = if source != PLAN_SECTION {
        format!("Added by {}; needs {}.", shape.owner, shape.needs)
    } else if shape.libstdcxx {
        format!("Exercises {} and {}.", shape.owner, shape.needs)
    } else {
        format!("Needs {} and {}.", shape.owner, shape.needs)
    };
    let _ = writeln!(
        out,
        "// Generated by `cargo xtask gen-reflect` (xtask/src/gen_reflect.rs): shape {upper}, \
         N={n}.\n\
         // Do not edit; regenerate. {source} {upper}: {}.\n// {needs}",
        shape.what
    );
    out.push_str("//@ std: c++26\n//@ args: -freflection\n");
    if shape.bare {
        out.push_str("//@ mode: emit-llvm\n");
    } else if !shape.libstdcxx {
        let _ = writeln!(out, "//@ xfail(kiln): {} (reflection family not landed)", shape.owner);
    }
    let _ = writeln!(out, "//@ oven: {}\n", (shape.budget)(n));
    if !shape.bare {
        out.push_str("#include <meta>\n");
    }
    (shape.body)(n, &mut out);
    out
}

/// Member types of the aggregate shapes, with their sizes (target-neutral:
/// the same on every target kiln supports).
const TYPES: [(&str, usize); 4] = [("int", 4), ("short", 2), ("long long", 8), ("char", 1)];

/// The 32-bit FNV-1a step the generated serializers use.
fn fnv(h: u32, b: u8) -> u32 {
    (h ^ u32::from(b)).wrapping_mul(16_777_619)
}

const FNV_BASIS: u32 = 2_166_136_261;

/// The digit member `j` of aggregate `i` is initialized to.
fn digit(i: usize, j: usize) -> usize {
    (i * 7 + j * 3) % 10
}

/// The serializer shared by S1 and S4: an FNV-1a digest of `name=digit;`
/// over the selected members.
fn serializer(out: &mut String, filter: Option<&str>) {
    let (open, close) = match filter {
        Some(key) => (
            format!(
                "    if constexpr (!std::meta::annotations_of_with_type(m, ^^{key}).empty()) {{\n"
            ),
            "    }\n",
        ),
        None => ("    {\n".to_owned(), "    }\n"),
    };
    let _ = write!(
        out,
        "\ntemplate <class T>\nconstexpr unsigned digest(const T& t) {{\n  unsigned h = {FNV_BASIS}u;\n  \
         auto mix = [&](char c) {{ h = (h ^ (unsigned char)c) * 16777619u; }};\n  template for \
         (constexpr auto m : std::define_static_array(\n                     \
         std::meta::nonstatic_data_members_of(^^T, std::meta::access_context::unchecked()))) {{\n\
         {open}      for (char c : std::meta::identifier_of(m)) mix(c);\n      mix('=');\n      \
         mix(char('0' + t.[:m:]));\n      mix(';');\n{close}  }}\n  return h;\n}}\n\n"
    );
}

/// S1 and S4: N aggregates of 16 digit-initialized members, each with a
/// `static_assert` on its digest; with `key`, every third member is
/// annotated and only those are serialized.
fn aggregates(n: usize, out: &mut String, key: Option<&str>) {
    if let Some(k) = key {
        let _ = writeln!(out, "\nstruct {k} {{}};");
    }
    serializer(out, key);
    for i in 0..n {
        let mut h = FNV_BASIS;
        let _ = write!(out, "struct A{i:04} {{");
        for j in 0..16 {
            let (ty, _) = TYPES[(i + j) % TYPES.len()];
            let d = digit(i, j);
            let selected = key.is_none() || j % 3 == 0;
            let ann = match key {
                Some(k) if selected => format!("[[={k}{{}}]] "),
                _ => String::new(),
            };
            let _ = write!(out, " {ann}{ty} m{j} = {d};");
            if selected {
                for b in format!("m{j}={d};").bytes() {
                    h = fnv(h, b);
                }
            }
        }
        let _ = writeln!(out, " }};\nstatic_assert(digest(A{i:04}{{}}) == {h}u);");
    }
}

fn s1(n: usize, out: &mut String) {
    aggregates(n, out, None);
}

fn s4(n: usize, out: &mut String) {
    aggregates(n, out, Some("key"));
}

fn s2(n: usize, out: &mut String) {
    out.push_str(
        "#include <string_view>\n\ntemplate <class E>\nconsteval std::string_view enum_name(E e) {\n  \
         template for (constexpr auto x : std::define_static_array(std::meta::enumerators_of(^^E)))\n    \
         if (e == [:x:]) return std::meta::identifier_of(x);\n  return {};\n}\n\n\
         template <class E>\nconsteval E enum_from(std::string_view s) {\n  \
         template for (constexpr auto x : std::define_static_array(std::meta::enumerators_of(^^E)))\n    \
         if (s == std::meta::identifier_of(x)) return [:x:];\n  return static_cast<E>(-1);\n}\n\n",
    );
    for i in 0..n {
        let _ = write!(out, "enum class E{i:04} : int {{");
        for k in 0..64 {
            let _ = write!(out, "{} v{k}", if k == 0 { "" } else { "," });
        }
        let (a, b) = ((i * 37) % 64, (i * 11 + 5) % 64);
        let _ = writeln!(
            out,
            " }};\nstatic_assert(enum_name(E{i:04}::v{a}) == \"v{a}\" && \
             enum_from<E{i:04}>(\"v{b}\") == E{i:04}::v{b});"
        );
    }
}

fn s3(n: usize, out: &mut String) {
    // The wide struct's specs are built in 10 evaluations of N (one
    // constexpr variable each) and defined in an eleventh, so no single
    // evaluation grows with the shape beyond N specs.
    let wide = 10 * n;
    let width = (wide - 1).to_string().len();
    let _ = write!(
        out,
        "#include <span>\n#include <string_view>\n#include <vector>\n\n\
         template <class T>\nusing column = T[8];\n\n\
         consteval void make_soa(std::meta::info soa, std::meta::info src) {{\n  \
         std::vector<std::meta::info> specs;\n  \
         for (auto m : std::meta::nonstatic_data_members_of(src, std::meta::access_context::unchecked()))\n    \
         specs.push_back(std::meta::data_member_spec(\n        \
         std::meta::dealias(std::meta::substitute(^^column, {{std::meta::type_of(m)}})),\n        \
         {{.name = std::meta::identifier_of(m)}}));\n  \
         std::meta::define_aggregate(soa, specs);\n}}\n\n\
         consteval std::vector<std::meta::info> wide_specs(int lo) {{\n  \
         std::vector<std::meta::info> specs;\n  for (int i = lo; i < lo + {n}; ++i) {{\n    \
         char name[16] = {{'w'}};\n    int k = 1;\n    \
         for (int p = 1{zeros}; p != 0; p /= 10) name[k++] = char('0' + i / p % 10);\n    \
         specs.push_back(std::meta::data_member_spec(^^int, {{.name = std::string_view(name, k)}}));\n  \
         }}\n  return specs;\n}}\n\n",
        zeros = "0".repeat(width - 1)
    );
    for c in 0..10 {
        let _ = writeln!(
            out,
            "constexpr std::span<const std::meta::info> wide{c} = \
             std::define_static_array(wide_specs({}));",
            c * n
        );
    }
    let parts: Vec<String> = (0..10).map(|c| format!("wide{c}")).collect();
    let _ = write!(
        out,
        "struct Wide;\nconsteval {{\n  std::vector<std::meta::info> all;\n  \
         for (auto part : {{{}}})\n    all.insert(all.end(), part.begin(), part.end());\n  \
         std::meta::define_aggregate(^^Wide, all);\n}}\n\
         static_assert(sizeof(Wide) == {} && sizeof(Wide::w{:0width$}) == 4);\n\n",
        parts.join(", "),
        4 * wide,
        wide - 1
    );
    for i in 0..n {
        let mut size = 0;
        let _ = write!(out, "struct P{i:04} {{");
        for (j, name) in ["a", "b", "c", "d"].iter().enumerate() {
            let (ty, sz) = TYPES[(i + j) % TYPES.len()];
            size += 8 * sz;
            let _ = write!(out, " {ty} {name};");
        }
        let _ = writeln!(
            out,
            " }};\nstruct P{i:04}_soa;\nconsteval {{ make_soa(^^P{i:04}_soa, ^^P{i:04}); }}\n\
             static_assert(sizeof(P{i:04}_soa) == {size});"
        );
    }
}

fn s5(n: usize, out: &mut String) {
    out.push_str(
        "\ntemplate <class T, int I>\nstruct cell {\n  using type = T;\n  static constexpr int value = I;\n};\n\n\
         constexpr std::meta::info kinds[] = {^^char, ^^short, ^^int, ^^long long};\n\n\
         consteval int chunk(int lo) {\n  int sum = 0;\n  for (int i = lo; i < lo + 100; ++i) {\n    \
         auto r = std::meta::substitute(^^cell, {kinds[i % 4], std::meta::reflect_constant(i)});\n    \
         auto args = std::meta::template_arguments_of(r);\n    \
         if (args[0] != kinds[i % 4]) return -1;\n    \
         auto statics = std::meta::static_data_members_of(r, std::meta::access_context::unchecked());\n    \
         sum += std::meta::extract<int>(args[1]) + std::meta::extract<int>(statics[0]);\n  }\n  \
         return sum;\n}\n\n",
    );
    for c in 0..n / 100 {
        let lo = c * 100;
        let sum: usize = (lo..lo + 100).map(|i| 2 * i).sum();
        let _ = writeln!(out, "static_assert(chunk({lo}) == {sum});");
    }
}

fn s6(n: usize, out: &mut String) {
    out.push_str("\nnamespace big {\n");
    for i in 0..n {
        let _ = match i % 3 {
            0 => writeln!(out, "int f{i:05}();"),
            1 => writeln!(out, "struct t{i:05};"),
            _ => writeln!(out, "extern int v{i:05};"),
        };
    }
    let fns = n.div_ceil(3);
    out.push_str(
        "}  // namespace big\n\nconsteval int scan(int k) {\n  int fns = 0;\n  \
         for (auto m : std::meta::members_of(^^big, std::meta::access_context::unchecked()))\n    \
         if (std::meta::is_function(m)) ++fns;\n  return fns + k;\n}\n\n",
    );
    for k in 0..100 {
        let _ = writeln!(out, "static_assert(scan({k}) == {});", fns + k);
    }
}

/// Member types of the binding shapes S7/S8: C++ spelling, the name the
/// binding signatures give it, an initializer.
const BIND_TYPES: [(&str, &str, &str); 5] = [
    ("int", "int", "1"),
    ("double", "float", "2.5"),
    ("long long", "int", "3"),
    ("short", "int", "4"),
    ("unsigned", "int", "5u"),
];

/// One class of the binding shapes: what it declares and what a binding
/// generator selects from it.
struct BindClass {
    i: usize,
    /// Annotated `bind::bound`; every eighth class is not, and is skipped.
    bound: bool,
    /// `(type index, readonly, doc)` per annotated field `f<j>`.
    fields: Vec<(usize, bool, bool)>,
    /// Getter/setter pairs for the first `accessors` fields.
    accessors: usize,
    /// An annotated `double scale(double, int) const`.
    scale: bool,
    /// An annotated `virtual int kind() const`.
    virt: bool,
}

impl BindClass {
    fn new(i: usize) -> Self {
        let fields = (0..2 + i % 4)
            .map(|j| ((i + j) % BIND_TYPES.len(), (i + j) % 3 == 1, j % 2 == 0))
            .collect();
        BindClass {
            i,
            bound: i % 8 != 7,
            fields,
            accessors: 1 + i % 2,
            scale: i.is_multiple_of(3),
            virt: i % 4 == 3,
        }
    }

    /// The declaration in `namespace lib`.
    fn declare(&self, out: &mut String) {
        let i = self.i;
        let bound = if self.bound { "[[=bind::bound{}]] " } else { "" };
        let _ = writeln!(out, "struct {bound}C{i:04} {{");
        for (j, &(t, ro, doc)) in self.fields.iter().enumerate() {
            let (ty, _, init) = BIND_TYPES[t];
            let ro = if ro { ", =bind::readonly{}" } else { "" };
            let doc = if doc { format!(", =bind::doc{{{j}}}") } else { String::new() };
            let _ = writeln!(out, "  [[=bind::field{{}}{ro}{doc}]] {ty} f{j} = {init};");
        }
        out.push_str("  int hidden = 0;\n");
        for j in 0..self.accessors {
            let ty = BIND_TYPES[self.fields[j].0].0;
            let _ = writeln!(
                out,
                "  [[=bind::method{{}}]] {ty} get{j}() const {{ return f{j}; }}\n  \
                 [[=bind::method{{}}]] void set{j}({ty} v) {{ f{j} = v; }}"
            );
        }
        if self.scale {
            out.push_str(
                "  [[=bind::method{}]] double scale(double k, int n) const { return f0 * k + n; }\n",
            );
        }
        if self.virt {
            let _ =
                writeln!(out, "  [[=bind::method{{}}]] virtual int kind() const {{ return {i}; }}");
        }
        let _ = writeln!(out, "  static int count() {{ return {i}; }}\n}};");
    }

    /// The binding text S7's generator emits for the class (and S8 compiles).
    fn binding(&self) -> String {
        let c = format!("C{:04}", self.i);
        let mut s = format!("  nb::class_<lib::{c}>(m, \"{c}\")\n    .def(nb::init<>())\n");
        for (j, &(_, ro, _)) in self.fields.iter().enumerate() {
            let def = if ro { "def_ro" } else { "def_rw" };
            let _ = writeln!(s, "    .{def}(\"f{j}\", &lib::{c}::f{j})");
        }
        let mut method = |name: &str, sig: &str| {
            let _ = writeln!(s, "    .def(\"{name}\", &lib::{c}::{name}) // {sig}");
        };
        for j in 0..self.accessors {
            let py = BIND_TYPES[self.fields[j].0].1;
            method(&format!("get{j}"), &format!("(self) -> {py}"));
            method(&format!("set{j}"), &format!("(self, {py}) -> None"));
        }
        if self.scale {
            method("scale", "(self, float, int) -> float");
        }
        if self.virt {
            method("kind", "(self) -> int");
        }
        s.push_str("  ;\n");
        s
    }
}

/// The annotation types and the N classes of S7 and S8.
fn bind_classes(n: usize, out: &mut String) -> Vec<BindClass> {
    out.push_str(
        "\nnamespace bind {\nstruct bound {};\nstruct field {};\nstruct readonly {};\n\
         struct method {};\nstruct doc { int id; };\n}  // namespace bind\n\nnamespace lib {\n",
    );
    let classes: Vec<BindClass> = (0..n).map(BindClass::new).collect();
    for c in &classes {
        c.declare(out);
    }
    out.push_str("}  // namespace lib\n");
    classes
}

/// S7: the generator TU. A consteval binding generator, a function
/// template per class whose `template for` walks `members_of` and
/// `annotations_of_with_type`, builds each class's binding as a
/// `std::string` (the bound-class list is a `std::vector`), and `main`
/// prints it from a `template for` over the classes that also splices
/// every bound member. Each class's text is checked against the
/// generator's.
fn s7(n: usize, out: &mut String) {
    out.push_str(
        "#include <cstdio>\n#include <string>\n#include <string_view>\n#include <type_traits>\n\
         #include <vector>\n",
    );
    let classes = bind_classes(n, out);
    let bound: Vec<&BindClass> = classes.iter().filter(|c| c.bound).collect();
    let _ = write!(
        out,
        r##"
constexpr auto ctx = std::meta::access_context::unchecked();

template <class Tag>
consteval bool has(std::meta::info r) {{
  return !std::meta::annotations_of_with_type(r, ^^Tag).empty();
}}

consteval std::vector<std::meta::info> bound_classes() {{
  std::vector<std::meta::info> out;
  for (auto m : std::meta::members_of(^^lib, ctx))
    if (std::meta::is_class_type(m) && has<bind::bound>(m)) out.push_back(m);
  return out;
}}

template <class T>
consteval std::string py_name() {{
  if constexpr (std::is_void_v<T>)
    return "None";
  else if constexpr (std::is_floating_point_v<T>)
    return "float";
  else if constexpr (std::is_integral_v<T>)
    return "int";
  else
    return std::string(std::meta::identifier_of(^^T));
}}

template <std::meta::info F>
consteval std::string signature() {{
  std::string s = "(self";
  template for (constexpr auto p : std::define_static_array(std::meta::parameters_of(F)))
    s += ", " + py_name<typename [:std::meta::type_of(p):]>();
  return s + ") -> " + py_name<typename [:std::meta::return_type_of(F):]>();
}}

template <std::meta::info C>
consteval std::string binding() {{
  std::string name(std::meta::identifier_of(C));
  std::string out = "  nb::class_<lib::" + name + ">(m, \"" + name + "\")\n";
  out += "    .def(nb::init<>())\n";
  template for (constexpr auto m : std::define_static_array(std::meta::members_of(C, ctx))) {{
    constexpr bool field = std::meta::is_nonstatic_data_member(m) && has<bind::field>(m);
    if constexpr (field || (std::meta::is_function(m) && has<bind::method>(m))) {{
      std::string id(std::meta::identifier_of(m));
      std::string ref = "(\"" + id + "\", &lib::" + name + "::" + id + ")";
      if constexpr (field)
        out += (has<bind::readonly>(m) ? "    .def_ro" : "    .def_rw") + ref + "\n";
      else
        out += "    .def" + ref + " // " + signature<m>() + "\n";
    }}
  }}
  return out + "  ;\n";
}}

constexpr unsigned digest(std::string_view s) {{
  unsigned h = {FNV_BASIS}u;
  for (char c : s) h = (h ^ (unsigned char)c) * 16777619u;
  return h;
}}

static_assert(bound_classes().size() == {count});

template <class T>
long long checksum(const T& obj) {{
  long long sum = 0;
  template for (constexpr auto m : std::define_static_array(std::meta::nonstatic_data_members_of(^^T, ctx)))
    if constexpr (has<bind::field>(m)) {{
      long long v = static_cast<long long>(obj.[:m:]);
      sum += v;
    }}
  return sum;
}}

int main() {{
  std::puts("#include \"binding.h\"\nvoid bind_all(nb::module_& m) {{");
  long long total = 0;
  template for (constexpr auto c : std::define_static_array(bound_classes())) {{
    constexpr const char* text = std::define_static_string(binding<c>());
    std::fputs(text, stdout);
    typename [:c:] obj{{}};
    total += checksum(obj);
  }}
  std::printf("}}\n// checksum %lld\n", total);
}}
"##,
        count = bound.len()
    );
    // Each class's binding text, checked in an evaluation of its own (so no
    // single evaluation grows with N).
    for c in bound {
        let text = c.binding();
        let _ = writeln!(
            out,
            "static_assert(digest(binding<^^lib::C{:04}>()) == {}u);",
            c.i,
            text.bytes().fold(FNV_BASIS, fnv)
        );
    }
}

/// S8's binding library: a self-contained miniature of a nanobind-style
/// `class_<T>` with `def`/`def_rw`/`def_ro`, type casters, constexpr
/// signature descriptors and type-erased call thunks.
const NB: &str = r#"
#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <utility>
#include <vector>

namespace nb {
struct handle {
  void* p = nullptr;
};

template <std::size_t N>
struct descr {
  char text[N + 1]{};
  constexpr descr() = default;
  constexpr descr(const char (&s)[N + 1]) {
    for (std::size_t i = 0; i < N; ++i) text[i] = s[i];
  }
  constexpr std::string_view view() const { return {text, N}; }
};

template <std::size_t N>
constexpr descr<N - 1> const_name(const char (&s)[N]) {
  return descr<N - 1>(s);
}

template <std::size_t A, std::size_t B>
constexpr descr<A + B> operator+(const descr<A>& a, const descr<B>& b) {
  descr<A + B> r;
  for (std::size_t i = 0; i < A; ++i) r.text[i] = a.text[i];
  for (std::size_t i = 0; i < B; ++i) r.text[A + i] = b.text[i];
  return r;
}

constexpr descr<0> concat() { return {}; }
template <std::size_t N>
constexpr descr<N> concat(const descr<N>& d) { return d; }
template <std::size_t N, class... Ds>
constexpr auto concat(const descr<N>& d, const Ds&... ds) {
  return d + const_name(", ") + concat(ds...);
}

namespace detail {
template <class T, class = void>
struct type_caster;

template <class T>
struct type_caster<T, std::enable_if_t<std::is_integral_v<T>>> {
  static constexpr auto Name = const_name("int");
  T value{};
  bool from_python(handle h) noexcept {
    value = static_cast<T>(reinterpret_cast<std::uintptr_t>(h.p));
    return true;
  }
  static handle from_cpp(T v) noexcept {
    return {reinterpret_cast<void*>(static_cast<std::uintptr_t>(v))};
  }
  T& get() noexcept { return value; }
};

template <class T>
struct type_caster<T, std::enable_if_t<std::is_floating_point_v<T>>> {
  static constexpr auto Name = const_name("float");
  T value{};
  bool from_python(handle h) noexcept {
    value = static_cast<T>(reinterpret_cast<std::uintptr_t>(h.p));
    return true;
  }
  static handle from_cpp(T v) noexcept {
    return {reinterpret_cast<void*>(static_cast<std::uintptr_t>(v))};
  }
  T& get() noexcept { return value; }
};

template <class T>
struct type_caster<T, std::enable_if_t<std::is_class_v<T>>> {
  static constexpr auto Name = const_name("self");
  T* value = nullptr;
  bool from_python(handle h) noexcept {
    value = static_cast<T*>(h.p);
    return value != nullptr;
  }
  static handle from_cpp(const T& v) noexcept { return {const_cast<T*>(&v)}; }
  T& get() noexcept { return *value; }
};

template <class T>
using make_caster = type_caster<std::remove_cvref_t<T>>;

template <class Ret>
constexpr auto return_name() {
  if constexpr (std::is_void_v<Ret>)
    return const_name("None");
  else
    return make_caster<Ret>::Name;
}

template <class Ret, class... Args>
constexpr auto signature() {
  return const_name("(") + concat(make_caster<Args>::Name...) + const_name(") -> ") +
         return_name<Ret>();
}

template <class Ret, class... Args, class F, std::size_t... Is>
handle call_impl(F& f, handle* args, std::index_sequence<Is...>) {
  std::tuple<make_caster<Args>...> in;
  if (!(std::get<Is>(in).from_python(args[Is]) && ...)) return {};
  if constexpr (std::is_void_v<Ret>) {
    f(std::get<Is>(in).get()...);
    return {};
  } else {
    return make_caster<Ret>::from_cpp(f(std::get<Is>(in).get()...));
  }
}
}  // namespace detail

struct function_record {
  std::string name;
  std::string signature;
  std::size_t nargs = 0;
  void* capture = nullptr;
  handle (*impl)(void*, handle*) = nullptr;
  void (*free_capture)(void*) = nullptr;
};

struct module_ {
  std::vector<function_record> functions;
  std::vector<std::string> types;
};

template <class... Args>
struct init {};

template <class Ret, class... Args, class F>
function_record make_function(std::string name, F f) {
  static constexpr auto sig = detail::signature<Ret, Args...>();
  function_record r;
  r.name = std::move(name);
  r.signature = std::string(sig.view());
  r.nargs = sizeof...(Args);
  r.capture = new F(std::move(f));
  r.impl = [](void* cap, handle* args) -> handle {
    return detail::call_impl<Ret, Args...>(*static_cast<F*>(cap), args,
                                           std::index_sequence_for<Args...>{});
  };
  r.free_capture = [](void* cap) { delete static_cast<F*>(cap); };
  return r;
}

template <class T>
class class_ {
  module_* m_;

 public:
  class_(module_& m, const char* name) : m_(&m) { m.types.emplace_back(name); }

  template <class... Args>
  class_& def(init<Args...>) {
    m_->functions.push_back(make_function<void, T&, Args...>(
        "__init__", [](T& self, Args... args) { self = T{args...}; }));
    return *this;
  }
  template <class Ret, class... Args>
  class_& def(const char* name, Ret (T::*f)(Args...)) {
    m_->functions.push_back(make_function<Ret, T&, Args...>(
        name, [f](T& self, Args... args) -> Ret { return (self.*f)(args...); }));
    return *this;
  }
  template <class Ret, class... Args>
  class_& def(const char* name, Ret (T::*f)(Args...) const) {
    m_->functions.push_back(make_function<Ret, const T&, Args...>(
        name, [f](const T& self, Args... args) -> Ret { return (self.*f)(args...); }));
    return *this;
  }
  template <class D>
  class_& def_ro(const char* name, D T::*pm) {
    m_->functions.push_back(make_function<D, const T&>(
        name, [pm](const T& self) -> D { return self.*pm; }));
    return *this;
  }
  template <class D>
  class_& def_rw(const char* name, D T::*pm) {
    def_ro(name, pm);
    m_->functions.push_back(make_function<void, T&, const D&>(
        name, [pm](T& self, const D& v) { self.*pm = v; }));
    return *this;
  }
};
}  // namespace nb
"#;

/// S8: the production TU, what S7's generator prints compiled against
/// [`NB`]: N `class_<T>` instantiations whose `def`s instantiate a call
/// thunk, a caster tuple and a constexpr signature per member, with each
/// signature checked against the comment S7 writes next to it.
fn s8(n: usize, out: &mut String) {
    out.push_str(NB);
    let classes = bind_classes(n, out);
    out.push_str("\nvoid bind_all(nb::module_& m) {\n");
    for c in classes.iter().filter(|c| c.bound) {
        out.push_str(&c.binding());
    }
    out.push_str("}\n\n");
    for c in classes.iter().filter(|c| c.bound) {
        let name = format!("C{:04}", c.i);
        for j in 0..c.accessors {
            let (ty, py, _) = BIND_TYPES[c.fields[j].0];
            let _ = writeln!(
                out,
                "static_assert(nb::detail::signature<{ty}, const lib::{name}&>().view() == \
                 \"(self) -> {py}\");"
            );
        }
    }
}

/// S9: N reflection-keyed instantiations, so the cost is template
/// instantiation, codegen and the mangling of `L u6__info <kind> <entity>
/// E` (spec/reflection.md §8.3). The types sit in a namespace (the
/// entity is a nested name) and every fourth also keys a template-id
/// three deep. Address tables odr-use every specialization.
fn s9(n: usize, out: &mut String) {
    out.push_str(
        "using info = decltype(^^int);\n\
         template <info R> int f() { static int n = 0; return ++n; }\n\
         template <info R> struct S { static int count; };\n\
         template <info R> int S<R>::count = 0;\n\
         template <class T> struct W {};\n\nnamespace lib {\n",
    );
    for i in 0..n {
        let _ = writeln!(out, "struct T{i:04} {{}};");
    }
    out.push_str("}  // namespace lib\n\nint (*fs[])() = {\n");
    for i in 0..n {
        let _ = writeln!(out, "  &f<^^lib::T{i:04}>,");
        if i % 4 == 0 {
            let _ = writeln!(out, "  &f<^^W<W<W<lib::T{i:04}>>>>,");
        }
    }
    out.push_str("};\n\nint* counts[] = {\n");
    for i in 0..n {
        let _ = writeln!(out, "  &S<^^lib::T{i:04}>::count,");
    }
    out.push_str("};\n");
}

/// S10: S9's pattern keyed on long nested types. Type `i` at depth `d`
/// is `lib::Node<d % 4><X(d-1), A1, ..., A5>`, the arguments drawn from
/// distinct leaves `lib::Tj`, `int`, `const lib::Ti*`, `long` and
/// `lib::Tj&`, so every name repeats subtrees (substitutions) and no two
/// keys are equal. The mangled length is linear in depth x width.
fn s10(n: usize, out: &mut String) {
    const DEPTH: usize = 6;
    const WIDTH: usize = 6;
    fn ty(n: usize, i: usize, d: usize, out: &mut String) {
        if d == 0 {
            let _ = write!(out, "lib::T{i:04}");
            return;
        }
        let _ = write!(out, "lib::Node{}<", d % 4);
        ty(n, i, d - 1, out);
        for a in 1..WIDTH {
            let j = (i * 7 + d * 13 + a) % n;
            let _ = match a % 5 {
                0 => write!(out, ", lib::T{j:04}"),
                1 => write!(out, ", int"),
                2 => write!(out, ", const lib::T{i:04}*"),
                3 => write!(out, ", long"),
                _ => write!(out, ", lib::T{j:04}&"),
            };
        }
        out.push('>');
    }
    out.push_str(
        "using info = decltype(^^int);\n\
         template <info R> int f() { static int n = 0; return ++n; }\n\
         template <info R> struct S { static int count; };\n\
         template <info R> int S<R>::count = 0;\n\nnamespace lib {\n",
    );
    for k in 0..4 {
        let _ = writeln!(out, "template <class... Ts> struct Node{k} {{}};");
    }
    for i in 0..n {
        let _ = writeln!(out, "struct T{i:04} {{}};");
    }
    out.push_str("}  // namespace lib\n\nint (*fs[])() = {\n");
    for i in 0..n {
        out.push_str("  &f<^^");
        ty(n, i, DEPTH, out);
        out.push_str(">,\n");
    }
    out.push_str("};\n\nint* counts[] = {\n");
    for i in 0..n {
        out.push_str("  &S<^^");
        ty(n, i, DEPTH, out);
        out.push_str(">::count,\n");
    }
    out.push_str("};\n");
}

pub const USAGE: &str = "\
usage: cargo xtask gen-reflect [--shape s1..s10] [--n <N>] [--out <dir>] [--check]

Writes the reflection perf shapes S1-S6 (docs/plans/m6-reflection.md §3.8) and
the reference-workload shapes S7-S8 (docs/status/perf-reference-workload.md) and
the reflection-mangling shapes S9 and S10 (spec/reflection.md §8.3) as bench entries. Without --n each shape is written at its committed default N
into bench/reflect/; a different N needs --out (the sweeps of `cargo xtask
bench --scaling` build their own). --check writes nothing and fails when a
committed file differs from the generator. See spec/bench.md \"Reflection shapes\".
";

/// `cargo xtask gen-reflect`.
pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    match run(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("xtask gen-reflect: {e}");
            ExitCode::from(2)
        }
    }
}

/// `Ok(false)`: `--check` found a stale file.
fn run(mut args: impl Iterator<Item = String>) -> Result<bool, String> {
    let (mut only, mut n, mut out, mut check) = (None, None, None, false);
    while let Some(a) = args.next() {
        let mut value = |f: &str| args.next().ok_or_else(|| format!("`{f}` requires a value"));
        match a.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(true);
            }
            "--shape" => {
                let v = value("--shape")?;
                only = Some(shape(&v).ok_or_else(|| format!("unknown shape `{v}` (s1..s10)"))?);
            }
            "--n" => {
                let v = value("--n")?;
                n = Some(
                    v.parse::<usize>()
                        .ok()
                        .filter(|&n| n > 0)
                        .ok_or_else(|| format!("bad --n `{v}` (a positive integer)"))?,
                );
            }
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--check" => check = true,
            f => return Err(format!("unknown argument `{f}`\n\n{USAGE}")),
        }
    }
    if n.is_some() && out.is_none() && !check {
        return Err(
            "--n other than the default needs --out (bench/reflect holds the defaults)".into()
        );
    }
    let dir = out.unwrap_or_else(|| crate::util::repo_root().join("bench").join("reflect"));
    let mut stale = Vec::new();
    for s in SHAPES.iter().filter(|s| only.is_none_or(|o| o.id == s.id)) {
        let n = n.unwrap_or(s.default_n);
        let path = write_or_check(&dir, s, n, check)?;
        if let Some(p) = path {
            stale.push(p.display().to_string());
        }
    }
    if !stale.is_empty() {
        eprintln!(
            "xtask gen-reflect: out of date (run `cargo xtask gen-reflect`): {}",
            stale.join(", ")
        );
    }
    Ok(stale.is_empty())
}

/// Writes (or with `check`, compares) `<dir>/<id>.cpp`; returns the path
/// when `check` finds it different.
pub fn write_or_check(
    dir: &Path,
    s: &Shape,
    n: usize,
    check: bool,
) -> Result<Option<PathBuf>, String> {
    if !n.is_multiple_of(s.step) {
        return Err(format!("shape {} needs N to be a multiple of {}", s.id, s.step));
    }
    let path = dir.join(format!("{}.cpp", s.id));
    let text = generate(s, n);
    if check {
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        return Ok((old != text).then_some(path));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same N, same bytes; and the committed `bench/reflect/` files are the
    /// generator's output at the default N.
    #[test]
    fn deterministic_and_committed() {
        let dir = crate::util::repo_root().join("bench").join("reflect");
        for s in &SHAPES {
            let a = generate(s, s.default_n);
            assert_eq!(a, generate(s, s.default_n), "{}", s.id);
            assert_ne!(a, generate(s, s.default_n + s.step), "{}", s.id);
            assert!(a.contains(&format!("N={}.", s.default_n)), "{}", s.id);
            assert_eq!(a.contains("//@ xfail(kiln): "), !s.libstdcxx && !s.bare, "{}", s.id);
            assert_eq!(a.contains("#include <meta>"), !s.bare, "{}", s.id);
            // S1–S6 compile against libstdc++ since WP-638; their header
            // names the families they exercise.
            let planned = s.source == PLAN_SECTION;
            assert_eq!(planned, ["s1", "s2", "s3", "s4", "s5", "s6"].contains(&s.id), "{}", s.id);
            assert!(!planned || s.libstdcxx, "{}", s.id);
            assert_eq!(a.contains(&format!("// Exercises {} and ", s.owner)), planned, "{}", s.id);
            assert_eq!(write_or_check(&dir, s, s.default_n, true), Ok(None), "{} is stale", s.id);
            let spec =
                crate::bench::entry::parse_spec(&a).unwrap_or_else(|e| panic!("{}: {e:?}", s.id));
            let xfail = spec.xfail_kiln.as_deref();
            assert!(
                s.libstdcxx || s.bare || xfail.is_some_and(|r| r.starts_with(s.owner)),
                "{}",
                s.id
            );
        }
        assert!(write_or_check(&dir, shape("S5").unwrap(), 150, true).is_err());
    }

    /// S8 compiles exactly the bindings S7 checks its generator against,
    /// and both see the same classes.
    #[test]
    fn s7_checks_what_s8_compiles() {
        let (s7, s8) = (generate(shape("s7").unwrap(), 9), generate(shape("s8").unwrap(), 9));
        let classes: String =
            (0..9).map(BindClass::new).filter(|c| c.bound).map(|c| c.binding()).collect();
        assert!(s8.contains(&classes));
        let c8 = BindClass::new(8).binding();
        let h = c8.bytes().fold(FNV_BASIS, fnv);
        assert!(s7.contains(&format!("digest(binding<^^lib::C0008>()) == {h}u")));
        assert!(s7.contains("bound_classes().size() == 8)"));
        assert!(!s7.contains("C0007\")") && s7.contains("struct C0007 {"));
        assert_eq!(kiln_flags(shape("s9").unwrap()), Vec::<String>::new());
        assert_eq!(kiln_flags(shape("s10").unwrap()), Vec::<String>::new());
    }

    #[test]
    fn digest_matches_fnv1a() {
        // FNV-1a 32 of "a" is 0xe40c292c.
        assert_eq!(fnv(FNV_BASIS, b'a'), 0xe40c_292c);
    }
}
