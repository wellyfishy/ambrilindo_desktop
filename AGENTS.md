# AGENTS.md — Ambrilindo Workspace

Last verified against source: 2026-10-05. If code and this file disagree, trust the code.

## 1. What is here

Two **separate Django projects** (each has its own git repo). They share no DB and no code. They talk only over HTTP.

| Path | Role | Runs where |
|---|---|---|
| `ambrilindo/ambrilindo/` | Local tournament scoring app (karate: tatami, bagan, jury, scoreboard) | Operator laptop, LAN only |
| `ambrilindo_web/ambrilindo_web_pub/` | Public portal (registration, rankings, live brackets) | Fly.io, `www.ambrilindo.com` |

Names used below: **LOCAL** = `ambrilindo`, **WEB** = `ambrilindo_web_pub`.

## 2. Stack and run

| | LOCAL | WEB |
|---|---|---|
| Python / Django | 3.x / 5.2.2 | 3.11 / 5.2 |
| Server | Daphne (ASGI) | gunicorn + UvicornWorker (ASGI) |
| DB | SQLite `db.sqlite3` | SQLite (WAL); `SQLITE_PATH=/data/db.sqlite3` on Fly |
| Channel layer | InMemory | Redis if `REDIS_URL`, else InMemory |
| Settings | `ambrilindo/settings.py` | `ambrilindo_web/settings.py` |
| venv | `ambrilindo/venv` | `ambrilindo_web/venv` |
| Tests | none (`backend/tests.py` empty) | none |

OS is Windows + PowerShell. Run commands:

```powershell
# LOCAL (port 8000). .env must set HOSTED_API_TOKEN or startup fails.
cd ambrilindo\ambrilindo; ..\venv\Scripts\python manage.py runserver

# WEB (port 8001 = LOCAL's default sync target). Set DEBUG=true for local dev.
cd ambrilindo_web\ambrilindo_web_pub; ..\venv\Scripts\python manage.py runserver 8001

# Checks before saying "done"
python manage.py check
python manage.py makemigrations --check   # only if models changed
```

Deploy (WEB only): `Dockerfile` + `fly.toml`. The container runs `migrate` on start.

## 3. Code map

Each project has one Django app, `backend/`.

**LOCAL**
- `backend/views.py` (~9,800 lines). All views live here. Search it by function name; do not read the whole file.
- `backend/consumers.py`: 9 WebSocket consumers, one per tatami screen. Routes are in `backend/routing.py`.
- `backend/sync_service.py`: pushes and pulls data to and from WEB. Also picks the sync target.
- `backend/utils.py`: `SyncQueue` worker, `send_to_hosted`, `get_kode_realtime`, round/final label helpers.
- `backend/apps.py`: `ready()` starts the SyncQueue retry thread.
- `database/sync_target.json`: target server picked at runtime (`local` / `production` / `custom`). This overrides `HOSTED_BASE_URL`.
- `database/event_mapping.json`: maps local event ids to WEB event ids.

**WEB**
- `backend/views/__init__.py` (~3,500 lines) plus `api.py`, `sync.py`, `auth.py`, `kontingen.py`, `landing.py`, `penataran.py`, `tenis_meja.py`.
- `backend/decorators.py`: `@role_required('admin' | 'kontingen')`.
- `backend/utils/email.py` (Resend) and `backend/utils/fonnte.py` (Meta WhatsApp Cloud API; the name is legacy).
- `backend/signals.py`: deletes the `realtime_bagan:{bagan_id}` cache entry when a DetailBagan is saved or deleted.
- SSE endpoints: `landing.py` `bagan_live_sse`, `event_roster_live_sse`.

## 4. Sync contract (most fragile area)

Auth header: `Authorization: Bearer <token>`.
- LOCAL sends `settings.HOSTED_API_TOKEN`.
- WEB checks `settings.API_TOKEN` in `check_auth()` (`views/api.py:1025`, uses `hmac.compare_digest`).
- **The two values must be the same.**

**Live path (queued):** views call `send_to_hosted_async()` (`utils.py`). That writes a `SyncQueue` row, and a single worker sends rows in FIFO order.
- `api/status/`, `api/result/`, `api/final-result/`, `api/edit-bagan/` → handlers in `views/api.py`.
- Rows are skipped unless `Event.is_live_sync_enabled=True`.
- A network error stops the queue (strict FIFO). An HTTP 400/404, or 5 retries, sets the row to `abandoned`.
- A retry thread runs every 15 s.

**Bulk path (direct `requests`, in `sync_service.py`):**

| LOCAL function | WEB endpoint | WEB handler (`views/sync.py`) |
|---|---|---|
| `fetch_hosted_events` | GET `api/sync/events/` | `list_events_api` |
| `pull_athletes_from_hosted` | GET `api/sync/export-atlet/<pk>/` | `export_event_athletes_api` |
| `push_bagan_to_hosted` | POST `api/sync/push-bagan/` | `receive_bagan_sync` |
| `force_sync_results_to_hosted` | POST `api/sync/force-results/` | `force_sync_results_api` |
| `push_roster_to_hosted` | POST `api/sync/push-roster/` | `receive_roster_sync` |
| `push_tatami_live_status` | POST `api/sync/tatami-status/` | `update_tatami_status_api` |

**Match key:** `kode_realtime = "{event_code}-{bagan.pk}-{detail_bagan.pk}"`, built by `get_kode_realtime()` in LOCAL `utils.py`. WEB stores it on `Bagan` and `DetailBagan`.

**Schemas differ. Do not mix them up.**

| | LOCAL | WEB |
|---|---|---|
| `DetailBagan.atlet1/atlet2` FK | `Atlet` | `DetailAtlet` |
| Bagan rankings | `juara_1..3b` + `peringkat_5..8` | `juara_1..3b` only |
| DetailBagan-only fields | `kode`, `hantei`, `team`, `assigned_tatami` | `kode_realtime`, `running`, `ring`, `tanggal`, `umpire`, ... |

If you change a payload field, update the sender (LOCAL) and the receiver (WEB) in the same change.

## 5. Auth and roles

- **LOCAL:** session login. `Role` is OneToOne with User; `role_type` is one of `admin`, `admin_tatami`, `jury`. View-only screens use the session keys `view_only_role`, `view_only_tatami_pk`, `view_only_event_pk` (set in `views.py` `auth()`). Auth checks are inline in each view.
- **WEB:** session login with `EmailBackend` (`backend/backends.py`). Roles are boolean flags on `UserType` (`is_admin`, `is_manager`, `is_wasit`, `is_admin_table`, ...). Template views use `@role_required`. Machine-to-machine endpoints use `check_auth()` and `@csrf_exempt`.

## 6. Conventions

- Domain terms are Bahasa Indonesia: `bagan` (bracket), `atlet`, `utusan` / `kontingen` (delegation), `perguruan` (club), `nomor_tanding` (event category), `tatami`, `wasit` (referee), `juara`. Keep these names; do not translate them.
- Almost every FK is `null=True`. Guard every dereference: `if obj.fk and obj.fk.field`.
- No DRF. Responses use `JsonResponse({'status': 'success', ...})` or `JsonResponse({'error': '...'}, status=4xx)`.
- Wildcard imports (`from .models import *`, `from .views import *`) are used everywhere. Check for name collisions before adding a new top-level name.
- Comments and log messages are often in Indonesian. Match the language of the surrounding code.
- Static files: WhiteNoise. WEB media goes to S3 when `AWS_STORAGE_BUCKET_NAME` and `AWS_ACCESS_KEY_ID` are set (`USE_S3=false` forces local storage).
- **UI Consistency Rule (MANDATORY)**: All views, channels, tables, and modal dialogs across the desktop app (`ambrilindo_desktop`) MUST strictly adhere to the unified design system established in `data-atlet` (`RosterView.svelte`):
  - **Outer Frame**: `h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative`.
  - **Header Bar**: Upper-case bold title (`text-xl font-black text-white tracking-tight`), count pill badge (`px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]`), subtitle (`text-xs text-zinc-400 mt-1 font-normal`), primary action button (`h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md`), secondary button (`h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl`).
  - **KPI Stats Cards**: 5-column responsive strip (`grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0`), card container (`bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between`), title (`text-[10px] font-bold tracking-wider uppercase`), number (`text-2xl font-black font-mono text-white`), unit suffix (`text-[10px] font-mono text-zinc-500`).
  - **Toolbar**: Quick status filter pills (`px-3 py-1.5 rounded-lg text-xs font-semibold transition border`), fuzzy select / select dropdowns, search input with clear button (`w-56 bg-[#181a20] text-xs text-white pl-8 pr-7 py-1.5 rounded-xl border border-white/[0.1] focus:border-white`).
  - **Bulk Action Floating Bar**: When items are checked (`bg-[#181a20] border border-white/20 rounded-xl px-4 py-2 mb-3 flex items-center justify-between shadow-lg`).
  - **Data Table**: Container (`flex-1 overflow-y-auto min-h-0 rounded-2xl border border-white/[0.08] bg-[#181a20] shadow-lg flex flex-col`), sticky table header (`bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider sticky top-0 z-10 border-b border-white/[0.08]`), row hover (`hover:bg-white/[0.02]`), row selection (`bg-white/[0.04]`), action icon buttons (`p-1.5 text-zinc-400 hover:text-white rounded-lg hover:bg-white/[0.08] transition`).
  - **Pagination Footer**: `bg-[#111215] border-t border-white/[0.08] px-4 py-2.5 flex items-center justify-between text-xs text-zinc-400 shrink-0`.
  - **Modals**: Backdrop (`fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs`), container (`bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl`), primary button (`h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md`).
  - **Never deviate or introduce random divergent UI styles unless the user explicitly requests changes.**
  - **No Redundant "AI-Style" Subtitles / Taglines (MANDATORY)**: DO NOT automatically pair every title, card, modal header, or button with a boilerplate explanatory subtitle or small tagline (e.g. Card: "Tatami 1" -> Tagline: "Buka panel wasit tatami 1" or Title: "Pilih Mode" -> Tagline: "Silakan pilih salah satu mode"). Tournament software is an operational console, not a marketing website. Keep headings, cards, and buttons utilitarian, crisp, confident, and direct. Only show secondary text when it conveys essential, dynamic domain data (e.g., athlete contingent under an athlete's name, or active filter status).

## 7. Rules for agents

1. **Verify before you claim.** Open or grep the file before you name a function, route, field or env var, and cite `path:line`. Mark anything you could not check as UNVERIFIED.
2. **COMPLEX changes need a plan and user approval before you edit.** A change is COMPLEX if it touches any of:
   - auth, sessions or roles
   - `check_auth` or either API token
   - any `/api/*` sync endpoint, or the sending code in `sync_service.py` / `utils.py`
   - a model change that needs a migration
   - email or WhatsApp sending
   - S3 or storage config
   - both projects at once

   Everything else is SIMPLE; go ahead.
3. **Risk scan every touched area:**
   - null-FK dereferences
   - bare `except Exception` that silently swallows errors
   - N+1 queries (use `select_related` / `prefetch_related`)
   - missing auth checks
   - secrets written to logs

   Report what you find under "Noticed while in here:". Do not fix unrelated items without asking first.
4. **Never print, log or commit secrets.** That covers `.env` values, tokens and keys. Neither `.env` file is in git; keep it that way.
5. **No throwaway test files.** Only write scratch scripts for external state (DB, network, files). Put them in a scratch folder and delete them afterwards.
6. **Say how you verified.** Before calling a change done, state which check you ran (`manage.py check`, `makemigrations --check`, or a manual test), or say that you ran none.
7. **Tool note:** ripgrep-based search can return nothing inside the `ambrilindo/` tree. Fall back to PowerShell `Select-String`.

## 8. Known risks (verified 2026-10-05)

- **CRITICAL** — WEB `backend/utils/email.py:11` has a hardcoded Resend API key as the fallback default. Rotate the key and remove the literal.
- **CRITICAL** — LOCAL `ambrilindo/settings.py:28` has a hardcoded `SECRET_KEY`, plus `DEBUG=True` and `ALLOWED_HOSTS=['*']`. Acceptable on a LAN only. Never deploy it as-is.
- **HIGH** — WEB `settings.py:36`: `API_TOKEN` falls back to a known default string if no env var is set. `settings.py:25` does the same for `META_WA_VERIFY_TOKEN`.
- **HIGH** — WEB `UserType.pw` (`models.py:360`) is a plain `CharField`. It may store plaintext passwords (usage UNVERIFIED).
- **MEDIUM** — `db.sqlite3` is tracked in git in both repos, as are LOCAL `database/*/db.sqlite3`. WEB `scratch/` is tracked even though `.gitignore` lists it.
- **MEDIUM** — WEB runs SQLite with 2 gunicorn workers. WAL mode plus a 20 s timeout reduces write-lock contention during live events but does not remove it.
- **LOW** — LOCAL `SyncQueue` uses the status `abandoned` (`utils.py:103`), which is not in `STATUS_CHOICES` (`models.py:311`).
- **LOW** — LOCAL `Event.event_code` is a property that writes to the DB when read (`models.py:14-19`).
