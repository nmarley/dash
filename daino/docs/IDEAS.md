# Ideas / Backlog

Low-priority improvements to revisit later.

## Auto-reload DB on file change (serve command)

When `dainod serve` is running and the LMDB data file is deleted,
replaced, or re-indexed, the server continues serving stale data from
the old memory-mapped pages. It should detect this and handle it:

- Watch `data.mdb` mtime or inode number on a timer (e.g., every 5s)
- If changed: close the old Env, reopen, swap the Arc<AppState>
- If deleted: log a warning, return 503 on all endpoints until it
  reappears
- Could also expose a `POST /api/reload` admin endpoint for manual
  trigger

## Other ideas

- Pagination for address tx/utxo endpoints
- Balance calculation endpoint (`/api/addr/{addr}/balance`)
- Block transaction list in block response (list of txids)
- WebSocket real-time event notifications
