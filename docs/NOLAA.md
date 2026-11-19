# NOLAA implementation plan and status

Draft 1.153 §12.2. This file tracks the migration from the conservative
intraprocedural heuristics in `src/sema/check.rs` to a full analysis.

## Implemented

- Heap units per function: allocation site/local, `manual`, abandoned, returned,
  escaped, explicit-disposed.
- Direct aliases (`local a = heapUnit`) and pointer aliases
  (`local p = &heapUnit`) with use-after-dispose propagation; aliases that could
  outlive the unit's lexical block are marked escaped (so no unsafe auto-dispose).
- Field alias: `holder.Field = &heapUnit` and pointer-member writes
  (`p->Field = &heapUnit`) enable `dispose holder->Field` / `dispose p->Field`;
  the same pattern feeds SCC/demand edges for mutually needed units.
- Local container element aliases (`bucket[i] = heapUnit`) keep the unit alive for
  the container's scope, allow block-end disposal, propagate escape through the
  container, and report use-after-dispose when the element is accessed later.
- Automatic disposal at block natural exits/fall-through and jump disposal at
  `return`/`break`/`continue`; SCC grouping for units needed through fields.
- Safe-point selection: the last direct demand statement of a unit's block is
  chosen as the insertion point (dispose immediately after it); nested uses,
  jump-disposal paths and block-always-return paths fall back to block end.
- Path-sensitive dispose state: `if`/`else-if`/`else`, `match` cases (including
  wildcard exhaustiveness), `while`/`do-while`/`for` merge definitely-vs-maybe
  disposed states; jump disposal is skipped when a unit may already be disposed
  on some path.
- Warnings: Double-Dispose, Early-Dispose (use after dispose), Never-Disposed
  (including escaped units and units disposed on only some paths), linkable-call
  abandonment.
- Interprocedural ownership for references returned by functions (default
  allocator, plus explicit-allocator class instances via the class-header slot).
- Interprocedural parameter disposal: a function whose top-level body directly
  disposes a class parameter owns that unit; call sites mark the argument unit
  disposed (so the caller does not free it again) and reject non-heap arguments;
  codegen frees the parameter through the class-header allocator or the default
  allocator.

## Remaining / next

1. **CFG-safe-point insertion**: fall-through disposal is at block end and branch
   state is merged structurally;
   the CFG/dominator analysis should select the latest point where no demand
   remains, including branch/loop joins, instead of relying on block-end and
   jump-disposal heuristics.
2. **Points-to**: arbitrary pointer-to-pointer, struct/array/container element
   aliases (`container[i] = unit`), and field paths through parameters need
   flow-sensitive alias sets; unknown containers must disable auto-disposal.
3. **Interprocedural summaries**: parameter/field writes, read-only parameters,
   returned ownership and escape effects computed to a fixed point over the call
   graph (currently a syntactic return/dispose pre-pass).
4. **Diagnostics from analysis**: produce all four warning classes from the
   analysis result rather than sequential checking state.
5. **Value-type explicit-allocator returns**: allocation-descriptor/registry
   design for structures (class instances already carry the allocator slot).
6. **Tests**: branch/loop safe points, pointer/container aliases, parameter
   escapes, SCC cycles, custom allocators, manual units.

## Architecture target

`src/sema/nolaa.rs`:

- `Cfg` (basic blocks over AST statements, dominators/post-dominators, loop and
  scope exits);
- `PointsTo` (`Ref` -> set of unit ids, `Unknown` sentinel; field-sensitive,
  index/container -> Unknown);
- `Summary` (per function: parameter aliases, writes, return units/allocator,
  escape set, abandon/dispose effects; SCC fixed point);
- backward demand analysis + forward disposed-state analysis;
- `NolaaResult { dispose_at: HashMap<NodeId, Vec<LocalId>>, jump_dispose, warnings }`.

`check.rs` keeps only registration of units/locals; codegen consumes
`dispose_at`/`jump_dispose`.
