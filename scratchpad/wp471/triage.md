# WP-471 triage
- ensure_layout (kiln_abi/src/layout/mod.rs:516): computes via Layouter for any record; callers:
  codegen fold.rs:68 (bridge impl), sema packed.rs:93,117, builtins.rs:2340, gnu.rs:424,
  oven_bridge.rs:84 (SemaBridge::ensure_layout, kiln_oven lib.rs:269) -> oven ty.rs:741,
  compile.rs:2849 (__datasizeof), vm/virt.rs:257,262; tests: kiln_oven tests common/nested, kiln_abi layout_rules.
  Plan: Option<&RecordLayout>, None unless definition has DefinitionBits::COMPLETE; trait returns Option.
- Completion point: DeclArena::complete_definition (kiln_ast decl.rs:2683), reached only from
  AstBuilder::complete_record_definition (sema decl.rs:7293, lambdas, vtable graph). Already pushes
  completed_records -> epoch.
- Oven Types: provisional Vec<OTy>; intern hit does Vec::contains; refresh at every evaluate (lib.rs:787,876)
  scans all provisional slots. Plan: FxHashSet + seen-epoch.
- kiln_ast reflect.rs:61 expect on > u32 entries. Callers of intern: builder.rs:2418 (^^ expr),
  sema reflect.rs:228,317,415 (WP-470 files). Plan: intern -> Option; builder stores b=0 (no value)
  -> Oven traps "too many reflections"; sema helper intern_reflection(r, loc) -> Result<_, Vec<Note>>.
  No -f knob for the limit -> unit test with ReflectionTable::with_limit.
- same_entity: none exists. conv/bases.rs:67 same_class does a==b || first(a)==first(b).
  Raw DeclId compare in sema reflect.rs:354 (first==first) and :381 (as_record(t) == Some(r)).
## Baseline (base 96dfea53f, harness profile, tests/stress/provisional-types-scale.cpp)
- 5,000 classes / 10,003 evaluations: wall 0.40-0.44 s, oven wall_us total 235,515; steps 0 total;
  per-eval wall first 1000 = 5.7 us, last 1000 = 46 us (O(n) refresh per eval).
- 10,000 classes / 20,003 evals: wall 1.48 s, oven wall_us 1,010,534 (4.3x for 2x N: quadratic).
## After (epoch + FxHashSet, harness profile)
- 1x: wall 0.12-0.15 s, oven wall_us 5,575; per-eval first 1000 = 3.1 us, last 1000 = 0.39 us.
- 2x: wall 0.22-0.26 s, oven wall_us 10,106 (linear).
