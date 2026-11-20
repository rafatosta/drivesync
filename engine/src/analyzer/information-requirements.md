# Forward-only information contract

File -> Context -> Decision -> Apply -> Operator. No feedback path to an
earlier stage or Detector, no Resolve reuse. Context owns complementary reads;
Decision consumes completed facts; Apply executes the selected sequence with
refreshed guards from actual operation results. If invalidated, stop and record
partial effects; no reanalysis request. A new independently emitted File is a
separate analysis. Inventory persists facts only.

| Information | Artifact | Purpose | Missing treatment |
|---|---|---|---|
| ID, synchronization, time, causal order | File | freshness | block incomparable mutation |
| Physical identity, previous location | File/FileEvidence | association | historical path evaluation without invented identity |
| Presence incl trash/outside/inaccessible | FileEvidence | creation/deletion | never assume absence |
| Name/path, parent | FileData/FileEvidence | rename/move | Context reads or blocks |
| Content digest, revision | FileEvidence | compare/guard | Context own stable read |
| Kind, MIME, size, times, link count | FileData | feasibility | block unsafe action |
| Capabilities, namespace completeness | FileEvidence | permission/occupancy | Context reads |
| Account/provider/roots | ContextScope | scoped I/O | block |
| Current opposite side and history | SubjectFacts / historical snapshot | compare | terminal insufficiency |
| Exact destination parent/name/occupancy | SubjectFacts | creation/move | block unknown |
| Parent/content relations | SubjectFacts | deterministic decision | unknown |
| Authorization | SubjectFacts/policy | trash | block |
| Semantic findings and intents | AnalysisDecision | plan | no mutation if blocked |
| Commands, guards, execution identity | Apply | execution | interrupt stale work |
| Actual results and checkpoints | Operator | recovery/baseline | no blind repeat |
| Receipts and verified baseline | Apply | durable handling | Err if persistence fails |

File evidence fields represent observable facts only, default NotCollected.
Detector and external producer migration is intentionally deferred. Rust struct
literals must be migrated before compiling; serde default only covers decoding.
The new models are conceptual requirements, not proof producers can supply them.
Algorithms remain TDD stubs. No Inventory/Operator implementation changes.
