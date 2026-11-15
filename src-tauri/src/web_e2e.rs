//! Desktop <-> web end-to-end tests: the real desktop (LAN router + sync queue on a private
//! copy of the tournament DB) against a real local Django web portal (throw-away SQLite DB,
//! own port, seeded by `web_e2e_seed.py`), including the web's SSE streams.
//!
//! They need the web project next to this repo (or `AMBRILINDO_WEB_DIR`) and its venv, so
//! they are `#[ignore]`d in the normal run. Run them with:
//!   cargo test --lib web_e2e -- --ignored --test-threads=1
//! Production is never contacted: the desktop's sync target is written into the DB copy's
//! folder and every rig asserts it points at 127.0.0.1.
//!
//! Case ids (A = auth, B = bulk, C = live) match the test plan.

use crate::qa_tests::{bagans, details, ids, play, playable};
use crate::sync::{test_auth, SyncService};
use crate::test_support::{server_on_real_copy, TestServer, WebServer, WEB_TEST_LEGACY_TOKEN};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Credentials, the auth pause and the queue lock are process-wide: one e2e test at a time.
static E2E: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const EVENT: i64 = 17;
const SPREAD: [usize; 16] = [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15];

struct Rig {
    web: WebServer,
    desk: TestServer,
    db_path: PathBuf,
    pool: sqlx::SqlitePool,
    event_a: i64,
    /// Queue rows above this id were made by the test (the copy holds old rows).
    queue_base: i64,
    kode_event: String,
}

fn license(web: &WebServer, which: &str) -> test_auth::Mode {
    test_auth::Mode::License {
        token: web.seed[which]["token"].as_str().unwrap().to_string(),
        device_id: web.seed["device"].as_str().unwrap().to_string(),
    }
}

fn legacy() -> test_auth::Mode {
    test_auth::Mode::Legacy(WEB_TEST_LEGACY_TOKEN.into())
}

async fn rig() -> Option<Rig> {
    let web = WebServer::start().await?;
    let desk = server_on_real_copy().await?;
    let db_path = desk.dir.join("db.sqlite3");
    assert!(web.base.starts_with("http://127.0.0.1:"), "e2e must only talk to a local web");
    SyncService::set_sync_config(Some(&db_path), "custom", Some(web.base.clone())).expect("sync target");
    assert_eq!(SyncService::get_sync_config(Some(&db_path)).url, web.base, "desktop must target the local web");
    let event_a = web.seed["event_a"].as_i64().unwrap();
    SyncService::set_event_mapping(Some(&db_path), EVENT, event_a, "E2E Event A", "custom").expect("mapping");
    let pool = desk.pool().await;
    // Only our event syncs; old queue rows of other events in the copy stay out of the way.
    sqlx::query("UPDATE backend_event SET is_live_sync_enabled = 0").execute(&pool).await.unwrap();
    SyncService::toggle_live_sync(&pool, EVENT, true).await.unwrap();
    let queue_base: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) FROM backend_syncqueue").fetch_one(&pool).await.unwrap();
    let kode_event: String = sqlx::query_scalar("SELECT COALESCE(NULLIF(kode_event, ''), 'EV' || id) FROM backend_event WHERE id = ?")
        .bind(EVENT)
        .fetch_one(&pool)
        .await
        .unwrap();
    test_auth::set(Some(license(&web, "main")));
    Some(Rig { web, desk, db_path, pool, event_a, queue_base, kode_event })
}

#[derive(Debug, Clone, PartialEq)]
struct QRow {
    id: i64,
    endpoint: String,
    status: String,
    error: Option<String>,
    payload: String,
}

impl Rig {
    fn kode(&self, bagan: i64, m: i64) -> String {
        format!("{}-{}-{}", self.kode_event, bagan, m)
    }

    async fn queue(&self) -> Vec<QRow> {
        let rows: Vec<(i64, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, endpoint, status, last_error, payload FROM backend_syncqueue WHERE id > ? ORDER BY id",
        )
        .bind(self.queue_base)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        rows.into_iter().map(|(id, endpoint, status, error, payload)| QRow { id, endpoint, status, error, payload }).collect()
    }

    /// Runs the queue until nothing is pending (or 30 rounds), like the worker would.
    async fn drain(&self) -> Vec<QRow> {
        for _ in 0..30 {
            let _ = SyncService::process_sync_queue(&self.pool, Some(&self.db_path)).await;
            let q = self.queue().await;
            if !q.iter().any(|r| matches!(r.status.as_str(), "pending" | "processing")) && !q.iter().any(|r| r.status == "failed") {
                return q;
            }
            if q.iter().any(|r| r.status == "failed") && SyncService::auth_status().blocked_reason.is_some() {
                return q;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        self.queue().await
    }

    /// Runs python in the web (`manage.py shell`); the code prints `J=<json>` once.
    fn web_py(&self, code: &str) -> Value {
        let prelude = "import json\nfrom backend.models import Bagan, DetailBagan, Tatami, Event\n\
def d(k):\n    x = DetailBagan.objects.filter(kode_realtime=k).first()\n    \
return None if x is None else dict(pk=x.pk, bagan=x.bagan_id, selesai=x.selesai, running=x.running, ring=x.ring, \
score1=x.score1, score2=x.score2, pemenang=x.pemenang, atlet1=x.atlet1_id, atlet2=x.atlet2_id, vr1=x.vr1, vr2=x.vr2)\n";
        let out = self.web.shell(&format!("{prelude}{code}"));
        let line = out.lines().rev().find_map(|l| l.strip_prefix("J=")).unwrap_or_else(|| panic!("web_py printed no J=:\n{out}"));
        serde_json::from_str(line).expect("web_py json")
    }

    /// Manual-draws one fresh kumite category (spread placement, so it can be played to the
    /// end) and returns its bagan. Prefers `lo..=hi` athletes.
    async fn fresh_bagan(&self, lo: i64, hi: i64) -> Value {
        let eligible = self.desk.ok("get_drawing_eligible_categories", json!({ "eventId": EVENT })).await;
        let cats = eligible.as_array().unwrap();
        let fits = |c: &&Value, lo: i64, hi: i64| {
            let n = c["athlete_count"].as_i64().unwrap_or(0);
            (lo..=hi).contains(&n) && c["finished_matches"].as_i64().unwrap_or(0) == 0
        };
        let cat = cats
            .iter()
            .find(|c| fits(c, lo, hi) && c["is_kumite"] == true)
            .or_else(|| cats.iter().find(|c| fits(c, lo, hi)))
            .or_else(|| cats.iter().find(|c| fits(c, 2, 16)))
            .expect("a category with 2..=16 athletes and no results");
        let cat_id = cat["id"].as_i64().unwrap();
        let athletes = ids(&self.desk.ok("get_manual_drawing_athletes", json!({ "categoryId": cat_id })).await);
        let slots: Vec<Value> = athletes
            .iter()
            .enumerate()
            .map(|(k, id)| {
                let i = SPREAD[k];
                json!({ "atlet_id": id, "match_num": i / 2 + 1, "position": i % 2 + 1 })
            })
            .collect();
        self.desk.ok("execute_manual_drawing_bagan", json!({ "eventId": EVENT, "categoryId": cat_id, "slots": slots, "hasVr": true })).await;
        // Drawing does not queue anything; keep the queue empty of unrelated rows anyway.
        bagans(&self.desk, EVENT).await.into_iter().find(|b| b["nomor_tanding_id"].as_i64() == Some(cat_id)).expect("new bagan")
    }

    async fn push(&self, bagan: &Value) {
        SyncService::push_bagan_to_hosted(&self.pool, Some(&self.db_path), EVENT, Some(vec![bagan["id"].as_i64().unwrap()]))
            .await
            .expect("push bagan");
    }

    async fn next_playable(&self, bagan: &Value) -> Option<Value> {
        details(&self.desk, bagan["id"].as_i64().unwrap()).await.into_iter().filter(playable).min_by_key(|m| (m["round"].as_i64(), m["urutan"].as_i64()))
    }

    async fn finish(&self, bagan: &Value, m: &Value) {
        let (code, body) = play(&self.desk, bagan, m).await;
        assert_eq!(code, 200, "save result: {body}");
    }

    /// A score saved while the bout is still running (not finished).
    async fn save_running(&self, bagan: &Value, m: &Value) {
        let input = json!({
            "match_id": m["id"], "bagan_id": bagan["id"], "score1": "1", "score2": "0",
            "pemenang": "", "vr1": false, "vr2": false, "hantei": false, "selesai": false,
        });
        let (code, body) = self.desk.cmd("save_match_result", json!({ "input": input })).await;
        assert_eq!(code, 200, "save running score: {body}");
    }

    async fn notify_running(&self, m: &Value) {
        self.desk.ok("notify_match_running", json!({ "matchId": m["id"], "tatami": 1 })).await;
    }

    fn sse_bagan_path(&self, web_bagan: i64) -> String {
        format!("/event/{}/live/{}/sse/", self.event_a, web_bagan)
    }

    fn sse_event_path(&self) -> String {
        format!("/events/{}/live-sse/", self.event_a)
    }

    async fn desk_match(&self, bagan: i64, round: i64, urutan: i64) -> Option<Value> {
        details(&self.desk, bagan).await.into_iter().find(|m| m["round"].as_i64() == Some(round) && m["urutan"].as_i64() == Some(urutan))
    }
}

fn statuses(q: &[QRow]) -> Vec<(String, String)> {
    q.iter().map(|r| (r.endpoint.clone(), r.status.clone())).collect()
}

// ---------------------------------------------------------------------------------------
// A. Auth
// ---------------------------------------------------------------------------------------

/// A1, A2, A5, A7: who may list events, and what a broken / missing credential does.
#[tokio::test]
#[ignore]
async fn web_e2e_a_auth_modes() {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };
    let ids_of = |v: Vec<crate::sync::HostedEventOption>| v.into_iter().map(|e| e.id).collect::<Vec<_>>();
    let event_b = r.web.seed["event_b"].as_i64().unwrap();

    // A1 legacy token: every event.
    test_auth::set(Some(legacy()));
    let all = ids_of(SyncService::fetch_hosted_events(&r.web.base).await.expect("legacy lists events"));
    assert!(all.contains(&r.event_a) && all.contains(&event_b), "A1 legacy sees every event: {all:?}");

    // A2 license: only granted events.
    test_auth::set(Some(license(&r.web, "main")));
    let granted = ids_of(SyncService::fetch_hosted_events(&r.web.base).await.expect("license lists events"));
    assert_eq!(granted, vec![r.event_a], "A2 a license sees only its granted events");

    // A5 broken token / token of another device: 401.
    test_auth::set(Some(test_auth::Mode::License { token: "not-a-token".into(), device_id: "d".repeat(64) }));
    let e = SyncService::fetch_hosted_events(&r.web.base).await.unwrap_err();
    assert!(e.contains("401"), "A5 bad token -> 401: {e}");
    test_auth::set(Some(test_auth::Mode::License {
        token: r.web.seed["main"]["token"].as_str().unwrap().into(),
        device_id: "e".repeat(64),
    }));
    let e = SyncService::fetch_hosted_events(&r.web.base).await.unwrap_err();
    assert!(e.contains("401"), "A5 device mismatch -> 401: {e}");

    // A7 no credentials: nothing is sent, the live row stays queued.
    test_auth::set(Some(license(&r.web, "main")));
    let bagan = r.fresh_bagan(5, 8).await;
    r.push(&bagan).await;
    test_auth::set(Some(test_auth::Mode::None));
    assert!(SyncService::fetch_hosted_events(&r.web.base).await.is_err(), "A7 no credentials -> no request");
    let m = r.next_playable(&bagan).await.unwrap();
    r.finish(&bagan, &m).await;
    let (synced, failed) = SyncService::process_sync_queue(&r.pool, Some(&r.db_path)).await.unwrap();
    assert_eq!((synced, failed), (0, 0));
    assert_eq!(statuses(&r.queue().await), vec![("api/result/".into(), "pending".into())], "A7 row waits for credentials");
    // ...and goes out once a license is there.
    test_auth::set(Some(license(&r.web, "main")));
    assert!(r.drain().await.iter().all(|q| q.status == "synced"), "A7 sent once credentials exist");
}

/// A3, A4: the web refuses this computer -> row failed, queue paused and halted; an admin
/// grant on the web plus "Coba lagi" sends it.
#[tokio::test]
#[ignore]
async fn web_e2e_a_refusals_pause_queue() {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };
    let bagan = r.fresh_bagan(9, 16).await;
    r.push(&bagan).await;
    let m = r.next_playable(&bagan).await.unwrap();

    // A4 revoked / expired / released device: 403, pause.
    for which in ["revoked", "expired", "released"] {
        test_auth::set(Some(license(&r.web, which)));
        r.notify_running(&m).await;
        let q = r.drain().await;
        let last = q.last().unwrap();
        assert_eq!(last.status, "failed", "A4 {which}: {q:?}");
        assert!(last.error.as_deref().unwrap_or("").contains("403"), "A4 {which} -> 403: {:?}", last.error);
        assert!(SyncService::auth_status().blocked_reason.is_some(), "A4 {which}: queue paused");
        assert_eq!(SyncService::process_sync_queue(&r.pool, Some(&r.db_path)).await.unwrap(), (0, 0), "A4 {which}: paused queue sends nothing");
        sqlx::query("DELETE FROM backend_syncqueue WHERE id = ?").bind(last.id).execute(&r.pool).await.unwrap();
    }

    // A3 event not granted: first row refused, the next one stays queued (no hammering).
    test_auth::set(Some(license(&r.web, "no_grant")));
    r.notify_running(&m).await;
    r.finish(&bagan, &m).await;
    let q = r.drain().await;
    assert_eq!(statuses(&q), vec![("api/status/".into(), "failed".into()), ("api/result/".into(), "pending".into())], "A3 halted after 403: {q:?}");
    assert!(q[0].error.as_deref().unwrap_or("").contains("403"), "A3 -> 403: {:?}", q[0].error);

    // The admin grants the event on the web ("Lisensi Desktop" page), then "Coba lagi".
    r.web
        .admin_license_action(&[
            ("submit-type", "izin-event-tambah".into()),
            ("license_pk", r.web.seed["no_grant"]["pk"].to_string()),
            ("event_pk", r.event_a.to_string()),
        ])
        .await;
    assert!(SyncService::retry_single_item(&r.pool, Some(&r.db_path), q[0].id).await.expect("A3 retry after grant"));
    let q = r.drain().await;
    assert!(q.iter().all(|x| x.status == "synced"), "A3 everything sent after the grant: {q:?}");
    let web = r.web_py(&format!("print('J=' + json.dumps(d('{}')))", r.kode(bagan["id"].as_i64().unwrap(), m["id"].as_i64().unwrap())));
    assert_eq!(web["selesai"], true, "A3 result arrived: {web}");
}

// ---------------------------------------------------------------------------------------
// B. Bulk
// ---------------------------------------------------------------------------------------

/// B1-B6: ping, push (full / selected / prune), pull athletes twice, force results, no mapping.
#[tokio::test]
#[ignore]
async fn web_e2e_b_bulk() {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };

    // B1
    let ping = SyncService::ping_server(&r.web.base).await;
    assert!(ping.success, "B1 ping: {}", ping.message);

    // B3 full push: the web holds exactly the desktop's bagans and bouts; SSE says so.
    let mut ev_sse = r.web.sse(&r.sse_event_path()).await;
    assert_eq!(ev_sse.status, 200, "event SSE");
    SyncService::push_bagan_to_hosted(&r.pool, Some(&r.db_path), EVENT, None).await.expect("B3 full push");
    ev_sse.expect("bagan_synced", 15).await;
    let desk: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM backend_bagan WHERE event_id = ?), \
                (SELECT COUNT(*) FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id WHERE b.event_id = ?)",
    )
    .bind(EVENT)
    .bind(EVENT)
    .fetch_one(&r.pool)
    .await
    .unwrap();
    let count_py = format!(
        "print('J=' + json.dumps([Bagan.objects.filter(event_id={0}).count(), DetailBagan.objects.filter(bagan__event_id={0}).count()]))",
        r.event_a
    );
    let web = r.web_py(&count_py);
    assert_eq!((web[0].as_i64().unwrap(), web[1].as_i64().unwrap()), desk, "B3 web = desktop (bagans, bouts)");

    // B3 selected push -> bagan stream gets bagan_updated.
    let bagan = r.fresh_bagan(9, 16).await;
    r.push(&bagan).await;
    let b_id = bagan["id"].as_i64().unwrap();
    let first = details(&r.desk, b_id).await.into_iter().next().unwrap();
    let wb = r.web_py(&format!("print('J=' + json.dumps(d('{}')['bagan']))", r.kode(b_id, first["id"].as_i64().unwrap())));
    let mut b_sse = r.web.sse(&r.sse_bagan_path(wb.as_i64().unwrap())).await;
    assert_eq!(b_sse.status, 200, "bagan SSE");
    r.push(&bagan).await;
    b_sse.expect("bagan_updated", 15).await;

    // B4 selected push never deletes; a full push prunes what the desktop no longer has.
    let before = r.web_py(&count_py)[0].as_i64().unwrap();
    // Delete one other category's bagans the way the app does (no results there).
    let (victim_cat, victim_count): (i64, i64) = sqlx::query_as(
        "SELECT b.nomor_tanding_id, COUNT(*) FROM backend_bagan b WHERE b.event_id = ? AND b.nomor_tanding_id IS NOT NULL \
         AND b.nomor_tanding_id != ? \
         AND NOT EXISTS (SELECT 1 FROM backend_detailbagan d JOIN backend_bagan b2 ON d.bagan_id = b2.id \
                         WHERE b2.nomor_tanding_id = b.nomor_tanding_id AND d.selesai = 1) \
         GROUP BY b.nomor_tanding_id LIMIT 1",
    )
    .bind(EVENT)
    .bind(bagan["nomor_tanding_id"].as_i64().unwrap())
    .fetch_one(&r.pool)
    .await
    .expect("a category without results to delete");
    r.desk.ok("delete_category_bagans", json!({ "eventId": EVENT, "categoryId": victim_cat })).await;
    r.push(&bagan).await;
    assert_eq!(r.web_py(&count_py)[0].as_i64().unwrap(), before, "B4 selected push keeps other bagans");
    assert!(victim_count >= 1);
    SyncService::push_bagan_to_hosted(&r.pool, Some(&r.db_path), EVENT, None).await.expect("B4 full push");
    // The web now holds exactly the desktop's bagans again (the deleted category, and the
    // bagan replaced by the manual drawing above, are gone).
    let desk_bagans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_bagan WHERE event_id = ?").bind(EVENT).fetch_one(&r.pool).await.unwrap();
    let after = r.web_py(&count_py)[0].as_i64().unwrap();
    assert_eq!(after, desk_bagans, "B4 full push prunes to the desktop's bagans");
    assert!(after < before, "B4 something was pruned ({before} -> {after})");

    // B2 pull athletes twice: the second pull adds nothing.
    let count_atlet = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM backend_atlet WHERE event_id = ?").bind(EVENT).fetch_one(&r.pool).await.unwrap()
    };
    SyncService::pull_athletes_from_hosted(&r.pool, Some(&r.db_path), EVENT).await.expect("B2 pull");
    let after_first = count_atlet().await;
    SyncService::pull_athletes_from_hosted(&r.pool, Some(&r.db_path), EVENT).await.expect("B2 pull again");
    assert_eq!(count_atlet().await, after_first, "B2 second pull creates no duplicates");

    // B5 force results (live sync off): the web gets every finished bout, the page reloads.
    SyncService::toggle_live_sync(&r.pool, EVENT, false).await.unwrap();
    let m1 = r.next_playable(&bagan).await.unwrap();
    r.finish(&bagan, &m1).await;
    let m2 = r.next_playable(&bagan).await.unwrap();
    r.finish(&bagan, &m2).await;
    assert!(r.queue().await.is_empty(), "live sync off queues nothing");
    SyncService::force_sync_results_to_hosted(&r.pool, Some(&r.db_path), EVENT).await.expect("B5 force results");
    b_sse.expect("bulk_sync_completed", 15).await;
    let web = r.web_py(&format!(
        "print('J=' + json.dumps([d('{}'), d('{}')]))",
        r.kode(b_id, m1["id"].as_i64().unwrap()),
        r.kode(b_id, m2["id"].as_i64().unwrap())
    ));
    let saved = r.desk_match(b_id, m1["round"].as_i64().unwrap(), m1["urutan"].as_i64().unwrap()).await.unwrap();
    for w in web.as_array().unwrap() {
        assert_eq!((w["selesai"].clone(), w["score1"].clone(), w["pemenang"].clone()), (json!(true), saved["score1"].clone(), json!("1")), "B5 {w}");
    }

    // B6 no mapping: an error before any request.
    SyncService::remove_event_mapping(Some(&r.db_path), EVENT).unwrap();
    assert!(SyncService::push_bagan_to_hosted(&r.pool, Some(&r.db_path), EVENT, None).await.is_err(), "B6 push needs a mapping");
    assert!(SyncService::force_sync_results_to_hosted(&r.pool, Some(&r.db_path), EVENT).await.is_err(), "B6 force needs a mapping");
}

// ---------------------------------------------------------------------------------------
// C. Live
// ---------------------------------------------------------------------------------------

/// C1-C7, C13: one bout from "running" to finished, then the whole bagan to its podium,
/// checked in the web DB and on both SSE streams.
async fn live_flow(mode: fn(&WebServer) -> test_auth::Mode) {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };
    test_auth::set(Some(mode(&r.web)));
    let bagan = r.fresh_bagan(9, 16).await;
    let b_id = bagan["id"].as_i64().unwrap();
    r.push(&bagan).await;
    let m = r.next_playable(&bagan).await.expect("a playable bout");
    let m_id = m["id"].as_i64().unwrap();
    let k = r.kode(b_id, m_id);
    let w0 = r.web_py(&format!("print('J=' + json.dumps(d('{k}')))"));
    let web_bagan = w0["bagan"].as_i64().unwrap();
    let mut b_sse = r.web.sse(&r.sse_bagan_path(web_bagan)).await;
    let mut e_sse = r.web.sse(&r.sse_event_path()).await;
    assert_eq!((b_sse.status, e_sse.status), (200, 200), "both SSE streams connect");

    // C2 the tatami starts the bout.
    r.notify_running(&m).await;
    let q = r.drain().await;
    assert_eq!(statuses(&q), vec![("api/status/".into(), "synced".into())], "C1/C2 status row sent: {q:?}");
    let running = b_sse.expect("running", 15).await;
    assert_eq!(running["db_pk"].as_i64(), w0["pk"].as_i64(), "C2 SSE names the bout");
    e_sse.expect("match_running", 15).await;
    let tat = e_sse.expect("tatami_updated", 15).await;
    assert_eq!(tat["is_active"], true);
    let w = r.web_py(&format!(
        "t = Tatami.objects.filter(event_id={}, tatami_number=1).first()\nprint('J=' + json.dumps([d('{k}'), t.is_active if t else None]))",
        r.event_a
    ));
    assert_eq!((w[0]["running"].clone(), w[0]["ring"].clone(), w[1].clone()), (json!(true), json!("1"), json!(true)), "C2 web: {w}");

    // C3 a running score is not sent (the web would end the bout early).
    r.save_running(&bagan, &m).await;
    assert_eq!(r.queue().await.len(), 1, "C3 running score queues nothing");
    assert!(b_sse.drain(800).await.is_empty(), "C3 no SSE for a running score");

    // C4 + C13 the bout finishes: score, winner moved to the next bout with its VR flag, tatami free.
    r.finish(&bagan, &m).await;
    let q = r.drain().await;
    assert!(q.iter().all(|x| x.status == "synced"), "C4 sent: {q:?}");
    let payload: Value = serde_json::from_str(&q.last().unwrap().payload).unwrap();
    assert!(payload["msg_id"].is_string() && payload["next_kode_realtime"].is_string(), "C4 payload carries msg_id and next bout: {payload}");
    let finished = b_sse.expect("finished", 15).await;
    assert!(finished["next_db_pk"].is_number(), "C4 SSE carries the next bout: {finished}");
    e_sse.expect("match_finished", 15).await;
    let next_k = payload["next_kode_realtime"].as_str().unwrap().to_string();
    let slot = payload["target_slot"].as_str().unwrap().to_string();
    let w = r.web_py(&format!(
        "t = Tatami.objects.filter(event_id={}, tatami_number=1).first()\nprint('J=' + json.dumps([d('{k}'), d('{next_k}'), t.is_active if t else None]))",
        r.event_a
    ));
    // Compare with what the desktop stored (kata scores differ from kumite ones).
    let saved = r.desk_match(b_id, m["round"].as_i64().unwrap(), m["urutan"].as_i64().unwrap()).await.unwrap();
    assert_eq!(
        (w[0]["selesai"].clone(), w[0]["running"].clone(), w[0]["score1"].clone(), w[0]["score2"].clone(), w[0]["pemenang"].clone()),
        (json!(true), json!(false), saved["score1"].clone(), saved["score2"].clone(), json!("1")),
        "C4 web bout: {w}"
    );
    let winner = w[0]["atlet1"].clone();
    let (slot_atlet, slot_vr) = if slot == "atlet1" { ("atlet1", "vr1") } else { ("atlet2", "vr2") };
    assert_eq!(w[1][slot_atlet], winner, "C4 winner moved into {slot} of the next bout: {w}");
    let next_desk = r.desk_match(b_id, m["round"].as_i64().unwrap() + 1, (m["urutan"].as_i64().unwrap() + 1) / 2).await.unwrap();
    assert_eq!(w[1][slot_vr], next_desk[slot_vr], "C13 VR flag of the next bout matches the desktop");
    assert_eq!(w[2], json!(false), "C4 tatami no longer active");

    // C6 a late "running" for a finished bout changes nothing.
    r.notify_running(&m).await;
    r.drain().await;
    let w = r.web_py(&format!("print('J=' + json.dumps(d('{k}')))"));
    assert_eq!((w["selesai"].clone(), w["running"].clone()), (json!(true), json!(false)), "C6 finished bout stays finished: {w}");

    // C7 the same message again (a resend after a lost reply) is dropped by msg_id.
    let auth = SyncService::auth_headers().unwrap();
    let resp = auth
        .apply(reqwest::Client::new().post(format!("{}/api/result/", r.web.base)))
        .header("Content-Type", "application/json")
        .body(q.last().unwrap().payload.clone())
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "already_processed", "C7 duplicate dropped: {body}");

    // C5 play the bagan to the end: podium on the web and on both streams.
    while let Some(next) = r.next_playable(&bagan).await {
        r.finish(&bagan, &next).await;
    }
    let q = r.drain().await;
    assert!(q.iter().all(|x| x.status == "synced"), "C5 every row sent: {:?}", statuses(&q));
    assert!(q.iter().any(|x| x.endpoint == "api/final-result/"), "C5 podium sent");
    b_sse.expect("final_result", 20).await;
    e_sse.expect("final_result", 20).await;
    let desk_bouts = details(&r.desk, b_id).await;
    let finished_kodes: Vec<String> = desk_bouts.iter().filter(|x| x["selesai"] == true).map(|x| r.kode(b_id, x["id"].as_i64().unwrap())).collect();
    let w = r.web_py(&format!(
        "ks = {finished_kodes:?}\nb = Bagan.objects.get(pk={web_bagan})\nprint('J=' + json.dumps([[k for k in ks if not (d(k) or {{}}).get('selesai')], b.juara_1_id, b.juara_2_id]))"
    ));
    assert_eq!(w[0], json!([]), "C5 every bout finished on the desktop is finished on the web");
    assert!(w[1].is_number() && w[2].is_number(), "C5 juara 1 and 2 on the web: {w}");
}

#[tokio::test]
#[ignore]
async fn web_e2e_c_live_flow_license() {
    live_flow(|w| license(w, "main")).await;
}

/// C14: the same flow with the legacy Bearer token.
#[tokio::test]
#[ignore]
async fn web_e2e_c_live_flow_legacy() {
    live_flow(|_| legacy()).await;
}

/// C8: web unreachable -> rows wait in order; when it is back they all arrive, in order.
#[tokio::test]
#[ignore]
async fn web_e2e_c_offline_keeps_fifo() {
    let _g = E2E.lock().await;
    let Some(mut r) = rig().await else { return };
    let bagan = r.fresh_bagan(9, 16).await;
    r.push(&bagan).await;
    r.web.stop();
    let mut played = Vec::new();
    for _ in 0..3 {
        let m = r.next_playable(&bagan).await.unwrap();
        r.finish(&bagan, &m).await;
        played.push(m["id"].as_i64().unwrap());
    }
    // The post-save tasks may still be trying the first row: wait until it gave up.
    let mut q = r.queue().await;
    for _ in 0..50 {
        let _ = SyncService::process_sync_queue(&r.pool, Some(&r.db_path)).await;
        q = r.queue().await;
        if q.first().is_some_and(|x| x.status == "failed") && q.iter().all(|x| x.status != "processing") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    assert_eq!(q.len(), 3);
    assert_eq!(q[0].status, "failed", "C8 first row failed (offline): {q:?}");
    assert!(q[1..].iter().all(|x| x.status == "pending"), "C8 later rows wait behind it: {q:?}");
    assert!(q.iter().all(|x| x.status != "abandoned"), "C8 offline never abandons");

    r.web.restart().await;
    let q = r.drain().await;
    assert!(q.iter().all(|x| x.status == "synced"), "C8 all sent after reconnect: {q:?}");
    let b_id = bagan["id"].as_i64().unwrap();
    let kodes: Vec<String> = played.iter().map(|m| r.kode(b_id, *m)).collect();
    let w = r.web_py(&format!("print('J=' + json.dumps([d(k)['selesai'] for k in {kodes:?}]))"));
    assert_eq!(w, json!([true, true, true]), "C8 web has every result");
}

/// C9: a bout the web does not know (bagan never pushed) is abandoned and does not block the
/// rows after it.
#[tokio::test]
#[ignore]
async fn web_e2e_c_unknown_bout_is_skipped() {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };
    let bagan = r.fresh_bagan(9, 16).await;
    let m = r.next_playable(&bagan).await.unwrap();
    r.finish(&bagan, &m).await; // bagan not on the web yet
    let q = r.drain().await;
    assert_eq!(statuses(&q), vec![("api/result/".into(), "abandoned".into())], "C9 404 -> abandoned: {q:?}");
    assert!(q[0].error.as_deref().unwrap_or("").contains("404"), "C9 {:?}", q[0].error);
    r.push(&bagan).await;
    let m2 = r.next_playable(&bagan).await.unwrap();
    r.finish(&bagan, &m2).await;
    let q = r.drain().await;
    assert_eq!(q.last().unwrap().status, "synced", "C9 the queue went on: {q:?}");
}

/// C10: live sync off for the event -> nothing is queued.
#[tokio::test]
#[ignore]
async fn web_e2e_c_live_sync_off_queues_nothing() {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };
    let bagan = r.fresh_bagan(9, 16).await;
    r.push(&bagan).await;
    SyncService::toggle_live_sync(&r.pool, EVENT, false).await.unwrap();
    let m = r.next_playable(&bagan).await.unwrap();
    r.notify_running(&m).await;
    r.finish(&bagan, &m).await;
    assert!(r.queue().await.is_empty(), "C10 live sync off");
}

/// C11: the worker and the post-save tasks running at once send every row exactly once.
#[tokio::test]
#[ignore]
async fn web_e2e_c_single_sender() {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };
    let bagan = r.fresh_bagan(9, 16).await;
    r.push(&bagan).await;
    test_auth::set(Some(test_auth::Mode::None)); // queue up without sending
    for _ in 0..4 {
        let m = r.next_playable(&bagan).await.unwrap();
        r.finish(&bagan, &m).await;
    }
    let rows = r.queue().await.len();
    assert!(rows >= 4, "C11 rows queued: {rows}");
    test_auth::set(Some(license(&r.web, "main")));
    let (a, b, c) = tokio::join!(
        SyncService::process_sync_queue(&r.pool, Some(&r.db_path)),
        SyncService::process_sync_queue(&r.pool, Some(&r.db_path)),
        SyncService::process_sync_queue(&r.pool, Some(&r.db_path)),
    );
    let sent = a.unwrap().0 + b.unwrap().0 + c.unwrap().0;
    let q = r.drain().await;
    assert!(q.iter().all(|x| x.status == "synced"), "C11 all sent: {q:?}");
    assert_eq!(sent, rows, "C11 each row sent exactly once");
}

/// C12: bouts inside a team match are not sent on their own (the web has only the parent).
#[tokio::test]
#[ignore]
async fn web_e2e_c_team_child_bouts_not_sent() {
    let _g = E2E.lock().await;
    let Some(r) = rig().await else { return };
    let child: Option<(i64, i64)> = sqlx::query_as(
        "SELECT d.id, d.bagan_id FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id \
         WHERE b.event_id = ? AND d.team = 1 AND d.selesai = 0 AND d.atlet1_id IS NOT NULL AND d.atlet2_id IS NOT NULL LIMIT 1",
    )
    .bind(EVENT)
    .fetch_optional(&r.pool)
    .await
    .unwrap();
    let Some((child_id, child_bagan)) = child else {
        eprintln!("skip C12: no open team child bout in the copy");
        return;
    };
    let input = json!({
        "match_id": child_id, "bagan_id": child_bagan, "score1": "2", "score2": "0",
        "pemenang": "1", "vr1": false, "vr2": false, "hantei": false, "selesai": true,
    });
    let (code, body) = r.desk.cmd("save_match_result", json!({ "input": input })).await;
    assert_eq!(code, 200, "{body}");
    assert!(r.queue().await.is_empty(), "C12 team child bout not queued");
}
