# Destructive operation gate

For move, replace or delete: inspect stable ID, recorded base, local fingerprint and remote revision; ensure same sync root, unchanged preconditions and durable journal. If any observation UNKNOWN or diverged, refuse and surface conflict. Preserve old content or trash when possible; revalidate immediately before write; verify afterward. Do not treat HTTP error as confirmed deletion.
