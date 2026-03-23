# Meridian Loom // 100 Runtime Improvements

> A research-backed innovation docket for a Meridian-native agent runtime that is governance-native, Solo->Org by default, dual-track in platform strategy, and aggressively low-level where the payoff is real.

## Research anchors

This docket is grounded in:

- current Loom truth in [README.md](/root/meridian-loom/README.md) and current Kernel truth in [LOOM_SPEC.md](/tmp/meridian-kernel/docs/LOOM_SPEC.md)
- The WebAssembly Component Model guide and WIT / Canonical ABI material from Bytecode Alliance: <https://component-model.bytecodealliance.org/>
- WASI roadmap and native async direction: <https://wasi.dev/roadmap>
- Wasmtime resource limiting and pooling allocation docs: <https://docs.wasmtime.dev/api/wasmtime/trait.ResourceLimiter.html>, <https://docs.wasmtime.dev/examples-fast-instantiation.html>, <https://docs.wasmtime.dev/api/wasmtime/enum.InstanceAllocationStrategy.html>
- Linux `io_uring(7)`: <https://man7.org/linux/man-pages/man7/io_uring.7.html>
- Linux `sched_ext` and BPF scheduler docs: <https://www.kernel.org/doc/html/latest/scheduler/sched-ext.html>, <https://docs.ebpf.io/linux/program-type/BPF_PROG_TYPE_STRUCT_OPS/sched_ext_ops/>
- Linux BPF ring buffer docs: <https://www.kernel.org/doc/html/next/bpf/ringbuf.html>
- seL4 capability and MCS scheduling docs: <https://docs.sel4.systems/Tutorials/capabilities.html>, <https://docs.sel4.systems/Tutorials/mcs.html>
- Firecracker microVM docs: <https://firecracker-microvm.github.io/>

## Defaults used in this docket

- Primary axis: `Solo -> Org`
- Platform strategy: `Dual-track`, with a Linux-first exploitation layer
- Assembly depth: `Aggressive low-level`, but only where the gain in control, latency, or metering justifies the complexity
- Truth boundary: Loom is still an experimental scaffold; OpenClaw still runs the live host

## How to read each item

Each item is compact but decision-bearing:

- `What`: the change
- `Why`: why it matters
- `Beyond`: why this is above mainstream agent-runtime design
- `Deps`: what it depends on
- `Risk`: the main downside
- `Build`: `Now`, `Next`, or `Later`
- `Platform`: `Linux-first`, `Portable`, or `Dual-track`
- `ASM`: `none`, `hot-path`, `supervisor-core`, or `aggressive`
- `Truth`: `repo-adjacent`, `design thesis`, or `speculative`

---

## Part I — 50 breakthrough concepts

### A. Core runtime model

1. **Agent ISA micro-ops** — What: compile every governed action into a typed micro-op stream rather than free-form “tool calls”; Why: deterministic replay, metering, and policy binding become native; Beyond: mainstream runtimes stop at session/tool abstractions; Deps: envelope normalization, capability ABI; Risk: overdesign if op taxonomy explodes; Build: Next; Platform: Dual-track; ASM: supervisor-core; Truth: design thesis.
2. **Workcell-native execution** — What: make a workcell, not a process, the primary runtime object; Why: scheduling, budget, memory, and audit attach to a durable unit; Beyond: most runtimes expose agents, not governed cells; Deps: capsule model, identity binding; Risk: conceptual weight; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
3. **Envelope-first runtime kernel** — What: all internal scheduler and worker transitions operate on normalized envelopes; Why: governance hooks stay first-class even inside the runtime; Beyond: avoids tool-first drift; Deps: stable envelope schema; Risk: verbosity in low-level paths; Build: Now; Platform: Dual-track; ASM: hot-path; Truth: repo-adjacent.
4. **Action lineage DAG** — What: track each action as a node in a runtime DAG with parentage, retries, forks, and merges; Why: makes replay, blame, and provenance native; Beyond: most runtimes only keep linear logs; Deps: canonical runtime event IDs; Risk: storage growth; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
5. **Split control plane / execution plane** — What: hard-separate policy, admission, and routing from worker execution; Why: easier isolation and resource control; Beyond: many agent runtimes blur orchestration and execution; Deps: supervisor boundaries; Risk: more moving pieces; Build: Now; Platform: Dual-track; ASM: supervisor-core; Truth: repo-adjacent.
6. **Deterministic failure classes** — What: every runtime failure maps to a bounded taxonomy instead of arbitrary exception blobs; Why: sanctions, retries, and parity become mechanical; Beyond: reduces “AI runtime mystery meat”; Deps: runtime event schema; Risk: taxonomy churn; Build: Now; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
7. **Time-sliced agent budget model** — What: treat CPU time, I/O slots, memory, and money as one budget surface; Why: forces real economic/runtime alignment; Beyond: most systems only meter tokens or dollars; Deps: metering hooks, scheduler integration; Risk: calibration complexity; Build: Next; Platform: Linux-first; ASM: hot-path; Truth: design thesis.
8. **Replay-first state model** — What: design runtime state so any action can be replayed with the same envelope, capability set, and scheduling context; Why: this is the substrate for proof and forensics; Beyond: current runtimes optimize for convenience, not replayability; Deps: event IDs, job ledger, capsule snapshots; Risk: lower ergonomics early; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
9. **Governance lanes** — What: classify actions as low-risk, budgeted, privileged, or sanction-sensitive at runtime admission time; Why: lets the scheduler choose isolation and review policy dynamically; Beyond: avoids binary “safe/unsafe” models; Deps: policy compiler; Risk: policy sprawl; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
10. **Policy snapshots as executable artifacts** — What: freeze policy state per action so later disputes use the exact runtime view at execution time; Why: eliminates drift arguments; Beyond: most systems only log final decisions, not policy state; Deps: policy serialization; Risk: larger artifacts; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
11. **Capability-native agent identity** — What: identities are bundles of allowed capabilities, not just names/IDs; Why: it aligns runtime routing with enforceable boundaries; Beyond: agent identity becomes materially useful; Deps: capability ABI; Risk: migration cost; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
12. **Runtime constitution layer** — What: embed a thin constitutional layer inside Loom that can be compiled from Kernel policy snapshots; Why: Solo->Org works without forcing full kernel deployment everywhere; Beyond: makes governance portable; Deps: policy compiler, embedded mode; Risk: divergence from kernel truth; Build: Later; Platform: Dual-track; ASM: supervisor-core; Truth: design thesis.

### B. Assembly-augmented runtime core

13. **Assembly ring-buffer scheduler core** — What: implement the hottest ring push/pop and wakeup paths in assembly; Why: maximal control over latency, cache behavior, and accounting; Beyond: most runtimes stop at Rust/C queues; Deps: io_uring-style submission/completion semantics; Risk: per-arch maintenance; Build: Later; Platform: Linux-first; ASM: aggressive; Truth: design thesis.
14. **Zero-copy envelope trampolines** — What: use assembly stubs for envelope marshalling between runtime core and workers/components; Why: fewer copies, clearer trust boundaries; Beyond: pushes governance metadata into the IPC fabric itself; Deps: stable ABI; Risk: portability burden; Build: Later; Platform: Dual-track; ASM: supervisor-core; Truth: design thesis.
15. **Cycle-level metering hooks** — What: inject assembly probes around key transitions to account for CPU cycles and syscall cost; Why: budget enforcement gets precise; Beyond: finer than process-level timers; Deps: perf counters / rdtsc strategy; Risk: architecture variance; Build: Later; Platform: Linux-first; ASM: aggressive; Truth: speculative.
16. **Memory quarantine fences** — What: assembly-backed memory copy and scrub primitives for untrusted worker outputs; Why: reduce leakage across isolation tiers; Beyond: explicit data hygiene at runtime edges; Deps: isolation ladder; Risk: complexity vs benefit; Build: Later; Platform: Dual-track; ASM: aggressive; Truth: speculative.
17. **Fast-path capability gate checks** — What: compile the cheapest approval/budget/sanction checks into assembly-optimized decision stubs; Why: reduces latency for the common path; Beyond: governance stops being “slow control plane”; Deps: policy JIT or compilation; Risk: auditability of generated paths; Build: Later; Platform: Linux-first; ASM: aggressive; Truth: design thesis.
18. **Scheduler context switch assist** — What: hand-optimized context bookkeeping for Loom scheduler slots; Why: better queue fairness and lower supervisor overhead; Beyond: agent runtime starts looking more like an OS scheduler; Deps: persistent scheduler state; Risk: hard debugging; Build: Later; Platform: Linux-first; ASM: aggressive; Truth: design thesis.
19. **Crypto receipt fast-path** — What: assembly-accelerated receipt hashing/signing for runtime events; Why: proof bundles stay cheap under load; Beyond: attestation becomes normal-path, not special mode; Deps: signing model; Risk: side-channel work; Build: Next; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
20. **Interrupt-style reactor loop** — What: design Loom’s event core more like a reactor than a CLI dispatcher; Why: better substrate for long-lived runtimes and many tiny tasks; Beyond: moves away from tool-command worldview; Deps: scheduler refactor; Risk: invasive change; Build: Later; Platform: Linux-first; ASM: supervisor-core; Truth: design thesis.
21. **Lock-free state ownership zones** — What: use assembly-assisted atomics for the small number of contested ownership paths; Why: keeps scheduler state predictable; Beyond: avoids generic mutex-heavy runtime design; Deps: state partitioning; Risk: memory-ordering bugs; Build: Later; Platform: Dual-track; ASM: aggressive; Truth: design thesis.
22. **NUMA-aware agent placement** — What: include low-level placement hints so hot workcells stay near their memory; Why: improves throughput for high-density hosts; Beyond: rare in agent runtimes; Deps: scheduler topology model; Risk: overfitting to large hosts; Build: Later; Platform: Linux-first; ASM: hot-path; Truth: speculative.
23. **Snapshot delta compressor in assembly** — What: optimize capsule/job snapshot diffing for fast clone/replay; Why: cheaper portable work units; Beyond: treats snapshots as first-class runtime objects; Deps: capsule snapshots; Risk: engineering cost; Build: Later; Platform: Dual-track; ASM: aggressive; Truth: speculative.
24. **Cross-arch micro-op backend** — What: keep a portable reference path, but support x86_64 and aarch64 assembly backends for the hottest runtime paths; Why: aggressive low-level does not have to mean single-arch; Beyond: practical dual-track low-level design; Deps: ISA abstraction layer; Risk: maintenance; Build: Later; Platform: Dual-track; ASM: aggressive; Truth: design thesis.

### C. Capability ABI and component model

25. **WIT-first capability contracts** — What: define capabilities as WIT packages/worlds, not ad hoc tool schemas; Why: stable language-neutral contracts; Beyond: turns tooling into true runtime components; Deps: component model adoption; Risk: upfront modeling effort; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
26. **Capability import matrices** — What: every component declares exactly which host capabilities it imports; Why: explicit least privilege; Beyond: richer than generic “tool permissions”; Deps: package metadata; Risk: verbosity; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
27. **Capability cost classes** — What: package capabilities with declared budget/latency bands; Why: scheduler and treasury can reason before execution; Beyond: mainstream runtimes rarely price capabilities formally; Deps: metering model; Risk: stale estimates; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
28. **Capability revocation receipts** — What: revoking a capability emits a signed receipt and invalidates cached runtime handles; Why: policy changes propagate safely; Beyond: revocation becomes auditable; Deps: signing and cache invalidation; Risk: stale workers; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
29. **Composable worlds** — What: compose multiple capability worlds into a capsule-specific world; Why: portable workcells become productizable; Beyond: one runtime can tailor itself per workcell; Deps: component composition; Risk: combinatorial complexity; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
30. **Versioned capability leases** — What: runtime grants temporary leases to capabilities with expiry and provenance; Why: easier revocation, easier incident handling; Beyond: better than static plugin enablement; Deps: job/capsule identity; Risk: bookkeeping cost; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
31. **Signed capability packs** — What: distribute capability bundles with signatures, constraints, and attestable metadata; Why: third-party extension without surrendering control; Beyond: package management meets runtime governance; Deps: registry, receipts; Risk: supply-chain complexity; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
32. **Interface-level resource hints** — What: capabilities declare memory, CPU, I/O, and network expectations in the ABI; Why: isolation and scheduling can be chosen before invocation; Beyond: component metadata drives runtime behavior; Deps: pack metadata; Risk: inaccurate hints; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
33. **Capability provenance graph** — What: every capability knows which pack, world, signer, and policy created it; Why: blame and trust become mechanical; Beyond: better than plugin names and versions; Deps: signed packs; Risk: data model complexity; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
34. **Capability shims for legacy tools** — What: wrap old tool-call surfaces behind the new ABI so Loom can migrate gradually; Why: innovation without isolation from reality; Beyond: bridges the ecosystem rather than ignoring it; Deps: adapter generator; Risk: lowest-common-denominator behavior; Build: Now; Platform: Dual-track; ASM: none; Truth: repo-adjacent.

### D. Isolation ladder

35. **Four-tier isolation policy** — What: trusted in-process, Wasm component, sandboxed process, microVM; Why: match cost to risk; Beyond: no one-size-fits-all sandbox; Deps: capability metadata; Risk: policy mistakes; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
36. **Policy-driven isolation escalation** — What: runtime can promote a capability from Wasm to microVM because of sanction sensitivity or data class; Why: context-aware defense; Beyond: isolation becomes governance-coupled; Deps: policy compiler; Risk: jitter; Build: Later; Platform: Linux-first; ASM: none; Truth: design thesis.
37. **Wasm first-class for low-risk capabilities** — What: default low-risk capability packs to components; Why: faster cold start and strong boundaries; Beyond: components become the baseline extension story; Deps: WIT contracts; Risk: language/tooling gaps; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
38. **MicroVMs only for heavy trust boundaries** — What: reserve Firecracker-class isolation for sensitive or multi-tenant workcells; Why: preserve efficiency; Beyond: avoids fetishizing microVMs everywhere; Deps: isolation policy engine; Risk: operational complexity; Build: Later; Platform: Linux-first; ASM: none; Truth: design thesis.
39. **Shared-nothing capsules** — What: capsules can opt into no shared filesystem, no shared memory, no shared sockets by default; Why: portable least privilege; Beyond: closer to capability OS behavior than app sandboxing; Deps: capsule model; Risk: friction for developers; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
40. **Cross-tier attestation chain** — What: every isolation tier emits attestable metadata so parity/audit can compare like for like; Why: proof surfaces survive heterogeneity; Beyond: debugability across tiers; Deps: receipt model; Risk: too much metadata; Build: Later; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
41. **Snapshot-boot microVM capsules** — What: use prewarmed snapshots for sensitive capsules; Why: microVM isolation without terrible cold starts; Beyond: stronger than plain process sandboxing; Deps: snapshot orchestration; Risk: snapshot drift; Build: Later; Platform: Linux-first; ASM: none; Truth: design thesis.
42. **Isolation debt accounting** — What: treat cheap-but-risky isolation choices as an explicit debt item in reports; Why: makes compromises visible; Beyond: no more silent security shortcuts; Deps: policy engine; Risk: noisy reports; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
43. **Data diode output mode** — What: one-way output-only capsules for high-risk tasks; Why: limits exfiltration patterns; Beyond: unusual in agent runtimes; Deps: restricted IPC and storage; Risk: narrow applicability; Build: Later; Platform: Linux-first; ASM: none; Truth: speculative.
44. **Capability-local seccomp synthesis** — What: generate seccomp profiles from declared capability imports; Why: system-call surface matches runtime contract; Beyond: policy -> kernel boundary; Deps: capability metadata; Risk: incomplete syscall modeling; Build: Later; Platform: Linux-first; ASM: none; Truth: design thesis.

### E. Scheduler and queue breakthroughs

45. **Envelope scheduler instead of process scheduler** — What: schedule envelopes/jobs by policy class and budget, not just workers; Why: governance-aware fairness; Beyond: scheduler understands the work, not just the worker; Deps: envelope-first kernel; Risk: complexity; Build: Next; Platform: Linux-first; ASM: supervisor-core; Truth: design thesis.
46. **Budget-period scheduling contexts** — What: borrow seL4 MCS-style budget/period thinking for workcells and capsules; Why: upper-bound execution becomes explicit; Beyond: better than “max jobs” toggles; Deps: scheduler state model; Risk: hard UX; Build: Later; Platform: Dual-track; ASM: supervisor-core; Truth: design thesis.
47. **Passive server execution mode** — What: some workers run on client-donated budget instead of their own; Why: shared services stop becoming invisible cost sinks; Beyond: directly inspired by seL4 passive servers; Deps: budget transfer model; Risk: deadlocks and starvation; Build: Later; Platform: Dual-track; ASM: none; Truth: speculative.
48. **Cross-core ordering ring** — What: use an MPSC parity/audit ring to preserve cross-CPU ordering for causally-linked actions; Why: audit/order correctness; Beyond: applies BPF ring-buffer motivation to Loom events; Deps: event stream redesign; Risk: contention; Build: Later; Platform: Linux-first; ASM: hot-path; Truth: design thesis.
49. **Policy hot-swap scheduler** — What: let policy classes remap queue priorities live without restarting runtime; Why: institution control without restart; Beyond: policy becomes dynamic scheduler input; Deps: stable scheduler contracts; Risk: oscillation; Build: Later; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
50. **Fairness proofs as scheduler outputs** — What: runtime emits evidence of why work was delayed, denied, or deprioritized; Why: fairness becomes inspectable; Beyond: most schedulers are opaque; Deps: stateful scheduler accounting; Risk: overhead; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.

---

## Part II — 50 execution-grade improvements

### F. Runtime state, queue, and job execution

51. **Job reservation / ack / nack** — What: add leased reservations so multiple supervisors can coordinate safely; Why: moves beyond single-process queue semantics; Beyond: necessary for hosted scheduler evolution; Deps: persistent job store; Risk: lease bugs; Build: Next; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
52. **Persistent scheduler state store** — What: replace file-scattered state with a canonical runtime-owned state DB or append-only log; Why: makes daemon restarts survivable; Beyond: current local file rehearsal becomes a real runtime substrate; Deps: schema design; Risk: migration burden; Build: Next; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
53. **Job lease expiry handling** — What: automatically recover orphaned jobs from crashed supervisors; Why: production scheduler prerequisite; Beyond: basic queue correctness; Deps: reservation model; Risk: duplicate execution; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
54. **Idempotent execution keys** — What: every action gets a replay-safe execution key; Why: avoids duplicate side effects; Beyond: crucial for retries and parity; Deps: envelope hash strategy; Risk: accidental over-deduplication; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
55. **Queue partitioning by policy class** — What: separate lanes for cheap, privileged, budget-heavy, and sanction-sensitive work; Why: less head-of-line blocking; Beyond: policy-driven runtime behavior; Deps: governance lanes; Risk: starvation; Build: Next; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
56. **Job compaction snapshots** — What: compress old job ledgers into snapshot bundles while preserving proofs; Why: long-lived runtime hygiene; Beyond: practical persistence strategy; Deps: snapshot format; Risk: losing debug detail; Build: Later; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
57. **Runtime-side dedupe cache** — What: identify equivalent work already in flight; Why: save cost and latency; Beyond: especially relevant for agent swarms; Deps: execution keys; Risk: false equivalence; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
58. **Admission control on artifact pressure** — What: runtime denies new work when audit/parity/state pressure exceeds bounds; Why: prevent observability from becoming a DOS vector; Beyond: resource-aware ops; Deps: state metrics; Risk: operator surprise; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.

### G. Resource control and observability

59. **Store-level Wasm limits by default** — What: make Wasmtime `ResourceLimiter` and pool settings first-class in every component lane; Why: resource caps become real, not comments; Beyond: takes advantage of existing runtime capabilities systematically; Deps: component embedding; Risk: partial coverage of host allocations; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
60. **Pooling allocator profiles** — What: offer “low-latency”, “balanced”, and “dense” instantiation profiles; Why: instantiate many small capability components cheaply; Beyond: better than one default runtime config; Deps: Wasmtime pooling allocator; Risk: tuning burden; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
61. **Per-capability fuel budgets** — What: fuel/epoch limits per invocation, not just per store; Why: predictable cutoffs; Beyond: ties compute budget to governance budget; Deps: Wasmtime fuel; Risk: developer friction; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
62. **eBPF runtime probes** — What: expose scheduler, I/O, queue, and parity events through BPF-friendly tracepoints; Why: deep Linux observability without invasive logging; Beyond: runtime can be inspected externally; Deps: Linux-first path; Risk: operator skill ceiling; Build: Later; Platform: Linux-first; ASM: none; Truth: design thesis.
63. **BPF ring-buffer event export** — What: export ordered runtime events through a shared MPSC ring for high-rate observation; Why: better than per-CPU fragmentation; Beyond: aligned with kernel ring-buffer design goals; Deps: event schema; Risk: implementation complexity; Build: Later; Platform: Linux-first; ASM: hot-path; Truth: design thesis.
64. **cgroup budget binding** — What: bind workcells/capsules to cgroup slices reflecting treasury and policy ceilings; Why: OS-enforced limits back runtime promises; Beyond: governance -> kernel path; Deps: Linux-first deployment; Risk: config overhead; Build: Next; Platform: Linux-first; ASM: none; Truth: design thesis.
65. **Memory class accounting** — What: separate code, model, queue, audit, parity, component, and snapshot memory; Why: operators can see where runtime bloat lives; Beyond: much more granular than RSS; Deps: runtime metrics plumbing; Risk: instrumentation cost; Build: Later; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
66. **I/O pressure scoring** — What: treat I/O backpressure as a first-class runtime health metric; Why: job delays become intelligible; Beyond: agent runtimes usually ignore I/O class; Deps: event loop instrumentation; Risk: false positives; Build: Next; Platform: Linux-first; ASM: hot-path; Truth: design thesis.
67. **Artifact residency tiers** — What: classify audit/parity/job artifacts into hot, warm, and cold storage; Why: proofs stay cheap to keep; Beyond: observability becomes scalable; Deps: snapshot/compaction; Risk: retrieval latency; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
68. **Operator-side budget heatmap** — What: render per-agent and per-capability budget burn visually in CLI/TUI and docs; Why: Solo->Org usability; Beyond: better than raw logs; Deps: metering pipeline; Risk: visual noise; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.

### H. Governance-native enforcement

69. **Native sanction enforcement in worker runtime** — What: move from preview/deny shell logic to actual runtime-level enforcement gates; Why: closes one of the biggest current gaps; Beyond: governance becomes execution reality; Deps: worker admission path; Risk: accidental lockouts; Build: Next; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
70. **Approval checkpoint tickets** — What: privileged actions require a runtime-consumable approval ticket, not just a boolean; Why: cleaner provenance and revocation; Beyond: more durable than transient decisions; Deps: authority ticket format; Risk: extra ceremony; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
71. **Budget gate pre-commit reservations** — What: reserve spend before work starts, reconcile after completion; Why: fewer race conditions; Beyond: treasury meets scheduler semantics; Deps: metering + reservation model; Risk: stranded reserves; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
72. **Policy drift alarms** — What: detect when execution used stale policy snapshots relative to kernel truth; Why: avoid silent divergence; Beyond: operationalizes truth discipline; Deps: policy versioning; Risk: alert fatigue; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
73. **Restriction provenance in deny paths** — What: every deny reports whether it came from sanction, approval, budget, or local isolation rule; Why: no mystery fails; Beyond: improves operator trust; Deps: decision model; Risk: verbose output; Build: Now; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
74. **Sanction severity -> isolation escalation** — What: more severe restriction classes automatically demand stronger execution boundaries; Why: policy affects how code runs, not just whether; Beyond: novel governance-runtime coupling; Deps: isolation ladder; Risk: policy complexity; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
75. **Remediation-only runtime mode** — What: a sanctioned agent can only run repair/verification capabilities in a narrow runtime lane; Why: sanctions become constructive; Beyond: richer than hard ban; Deps: capability classes; Risk: loopholes; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
76. **Policy proofs bound to artifacts** — What: every execution artifact stores the exact policy snapshot hash that admitted it; Why: later audits become mechanical; Beyond: closes policy provenance gap; Deps: artifact schema; Risk: larger metadata; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.

### I. Audit, proof, and attestation

77. **Canonical runtime event schema v1** — What: formalize one event model for queue, decision, execution, audit, and parity; Why: every proof surface gets simpler; Beyond: today’s artifacts are related but not fully normalized; Deps: event model review; Risk: migration cost; Build: Next; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
78. **Signed runtime receipts** — What: sign execution receipts and job state transitions; Why: tamper evidence for dispute cases; Beyond: stronger than plain JSON logs; Deps: key management; Risk: key handling burden; Build: Later; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
79. **Proof bundle generator inside Loom** — What: let the runtime emit a public proof bundle directly, not only via kernel-side examples; Why: better runtime ownership of evidence; Beyond: runtime becomes self-explaining; Deps: canonical schema; Risk: duplicated logic; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
80. **Artifact cross-linking** — What: every artifact references the others by stable IDs, not just paths; Why: portability and future remote storage; Beyond: local files stop being the only truth; Deps: event IDs; Risk: migration burden; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
81. **Attested parity claims** — What: parity reports can optionally carry signer metadata and claim scope; Why: easier to publish external proof without overclaiming; Beyond: stronger public truth surfaces; Deps: signing model; Risk: false authority if abused; Build: Later; Platform: Dual-track; ASM: hot-path; Truth: design thesis.
82. **Reconstructable execution windows** — What: bundle all artifacts needed to reconstruct a bounded slice of runtime history; Why: incident response and proof; Beyond: not just tails of logs; Deps: snapshot packaging; Risk: storage cost; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
83. **Cross-host receipt gossip** — What: hosts can exchange signed receipts for federation disputes; Why: future federation readiness; Beyond: moves parity and proof into networked territory; Deps: host identity and transport; Risk: trust complexity; Build: Later; Platform: Dual-track; ASM: none; Truth: speculative.
84. **Claim-to-evidence linting** — What: docs and runtime status surfaces fail if wording outruns measured truth; Why: protects against hype drift; Beyond: codifies honesty; Deps: docs tooling; Risk: tooling annoyance; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.

### J. Solo -> Org product path

85. **One-command embedded bootstrap** — What: `loom init` should provision the minimum useful governed environment for a solo builder; Why: Solo->Org promise starts here; Beyond: removes conceptual tax; Deps: embedded mode; Risk: false simplicity; Build: Next; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
86. **First governed cell tutorial** — What: interactive path that creates one useful governed workcell, not just config files; Why: the user needs an aha moment; Beyond: better than dry quickstarts; Deps: capsule templates; Risk: maintenance; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
87. **Builder-grade simulation fixtures** — What: ship realistic policy, sanction, and budget fixtures for local experimentation; Why: Solo users can feel institutional mechanics locally; Beyond: bridges prototype and production concepts; Deps: rehearsal expansion; Risk: fixture rot; Build: Next; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
88. **Team handoff capsules** — What: package a local workcell into a shareable governed bundle for another operator; Why: clean ladder from solo to team; Beyond: more meaningful than config export; Deps: capsule model; Risk: secret handling; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
89. **Institution upgrade path** — What: preserve the same operator grammar when moving from embedded local mode to a remote kernel; Why: no mental model reset; Beyond: lowers adoption cliff; Deps: bridge design; Risk: hidden complexity; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
90. **Shadow-to-live onboarding** — What: a standard path from rehearsal-only to real hosted execution; Why: makes progress legible; Beyond: closes the “interesting but not usable” trap; Deps: hosted supervisor path; Risk: premature promotion; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.
91. **Opinionated profiles** — What: `solo`, `builder`, `team`, `institution` profiles that tune isolation, proofs, and scheduling; Why: good defaults; Beyond: config becomes productized; Deps: profile compiler; Risk: too many profiles; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
92. **Portable capsule demos** — What: sample capsules that show low-risk, medium-risk, and high-risk patterns; Why: teach the runtime model through artifacts; Beyond: better than docs alone; Deps: capsule packaging; Risk: sample sprawl; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.

### K. Terminal and operator experience

93. **TTY layout engine** — What: move from plain text sections to a real terminal layout system for state, counters, and proof blocks; Why: operator surface stops feeling like help text; Beyond: terminal becomes product surface; Deps: stable job/scheduler state; Risk: terminal portability; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
94. **Color policy by semantic layer** — What: map governance, runtime, parity, sanction, and budget states to a stable palette; Why: fast scanning without losing no-color compatibility; Beyond: consistent cross-surface language; Deps: operator grammar; Risk: visual clutter; Build: Now; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
95. **Artifact peek commands** — What: `loom job inspect`-style surfaces should exist for audit, parity, scheduler, and capsule state; Why: no more catting JSON paths manually; Beyond: operators get coherent views; Deps: canonical state schemas; Risk: command sprawl; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
96. **Decision trace rendering** — What: show exactly how sanction, approval, and budget checks combined; Why: eliminate opaque deny paths; Beyond: governance is inspectable in real time; Deps: normalized decision graph; Risk: verbosity; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
97. **Proof-first status screens** — What: `loom status` and `parity report` should foreground what is proven, simulated, or future; Why: truth discipline at the point of use; Beyond: runtime refuses to be marketingware; Deps: truth metadata; Risk: longer output; Build: Now; Platform: Dual-track; ASM: none; Truth: repo-adjacent.
98. **Temporal operator views** — What: add time-windowed views for jobs, parity, and budget burn; Why: operators need trends, not just latest state; Beyond: closer to a system console than CLI utilities; Deps: event model; Risk: storage and complexity; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.

### L. Transport and replacement path

99. **Execution-stream parity with OpenClaw** — What: compare Loom’s per-action execution stream against real OpenClaw action execution, not just live probes; Why: real retirement gate; Beyond: necessary to stop pretending parity exists when it does not; Deps: OpenClaw stream adapter; Risk: complexity and flakiness; Build: Next; Platform: Dual-track; ASM: none; Truth: design thesis.
100. **Transport-neutral runtime ingress** — What: define one internal ingress protocol that Telegram/MCP/A2A/HTTP all compile into; Why: transport replacement stops being bespoke; Beyond: Loom can replace OpenClaw without cloning its channel architecture; Deps: internal ingress ABI; Risk: abstraction mistakes; Build: Later; Platform: Dual-track; ASM: none; Truth: design thesis.

---

## Top 15 near-term build list

1. Job reservation / ack / nack
2. Persistent scheduler state store
3. Canonical runtime event schema
4. Native sanction enforcement in the runtime path
5. Budget gate pre-commit reservations
6. Store-level Wasm limits by default
7. Pooling allocator profiles
8. One-command embedded bootstrap
9. First governed cell tutorial
10. Artifact cross-linking by stable IDs
11. Proof-first status screens
12. Capability shims for legacy tools
13. Queue partitioning by policy class
14. Opinionated operator profiles
15. Execution-stream parity with OpenClaw

## Top 15 moonshot bets

1. Agent ISA micro-ops
2. Runtime constitution layer
3. Assembly ring-buffer scheduler core
4. Zero-copy envelope trampolines
5. Cycle-level metering hooks
6. Fast-path capability gate compilation
7. Passive server execution mode
8. Cross-core ordering ring
9. Snapshot-boot microVM capsules
10. Capability provenance graph
11. Signed capability packs
12. Cross-host receipt gossip
13. Data diode output mode
14. NUMA-aware agent placement
15. Hybrid cross-arch micro-op backend

## Top 10 anti-patterns

1. Rewriting OpenClaw in Rust and calling it innovation.
2. Using assembly everywhere without a measurable control or latency win.
3. Treating governance as an after-the-fact middleware instead of runtime semantics.
4. Building only for institutions and forgetting the Solo->Org ladder.
5. Using microVMs for every capability instead of an isolation ladder.
6. Shipping parity language that outruns actual evidence.
7. Keeping capability contracts implicit or undocumented.
8. Letting audit and parity devolve into unstructured file dumps.
9. Making the operator shell an afterthought.
10. Optimizing cold-start benchmarks before designing runtime ownership and proof boundaries.
