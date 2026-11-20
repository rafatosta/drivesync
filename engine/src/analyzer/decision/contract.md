# Decision contract (behavior-first)

The incoming AnalysisContext is the contract. Inventory is only a future storage
adapter for these facts; its current schema must not constrain this stage.

## Rules and safety
- No I/O, time, random IDs or state mutation in Decision. Fixed policy enters
  through DecisionEngine construction; decisions are reproducible.
- Identity, presence, content, name and parent are separate facts.
- A detector event label is never an operation. A missing record is never absence.
- A scope evaluates retained measurements; it cannot prove current physical state
  without a relevant observation. A newer File invalidates any measurement that
  depends on the changed payload. Apply must recheck before effects.
- Confirmed conflict blocks all intents for that subject; missing relevant
  evidence also blocks execution, but not other independent subjects.
- Neither default deletion policy grants permission. An explicit authorization
  policy may permit Trash after confirmed absence and unchanged surviving state.
  Permanent deletion is outside this contract.
- New linked, equivalent items and convergent changes require baseline
  confirmation; Decision never writes or confirms the baseline.
- Unsupported known kinds are a durable blocked outcome, not an endless query.

## Deferred integration requirements (not claims about current File)
The four positive one-sided creation fixtures assume retained explicit evidence
of absence at the intended destination and a known subject. An isolated new
File cannot establish opposite-side absence. The producer of remote namespace
absence for a previously unknown ID, and how Apply records the first observation
then requests a targeted verification, must be defined before integrating this
flow. The read port will eventually supply these facts independently of schema.

Content relations in historical fixtures assume prior verified measurements
(e.g. verified transfer). LocalFile has no hash. The Decision must not infer
byte equivalence from size/timestamps or remote version; a new payload lacking
applicable content evidence requires VerifyContent. Define evidence collection
and its lifetime before enabling that flow in production.

The current Context cannot distinguish a definite identity match plus occupied
destination from ambiguous association, nor prove physical destination occupancy.
D10 therefore tests conservative identity blocking, not a proven collision.
The occupied-destination conflict oracle remains pending until that fact is
represented; Apply must still protect the physical destination.

The current Context also needs to locate a retained item for a path-only absence
without creating a new link; the deletion rules here use retained confirmed
states, not an assumed local ID on Deleted envelopes.

## Running the RED suite

cargo test -p drivesync-engine --test analyzer_contracts decision_behavior -- --ignored

Behavior tests remain ignored until explicitly run or implemented. The concrete
entry point intentionally panics at todo!(). Structural artifact tests are active.
