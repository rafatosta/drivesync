# Crash recovery

Enumerate nonterminal operations. Determine whether external effects happened using stable IDs and versions; reconcile ambiguous remote outcomes before retry. Recover staging files and resumable sessions. Never delete corrupt ledger: preserve snapshot, block destructive actions, rebuild with explicit safe reconciliation. Recover pending cursor only after staged remote changes were committed.
