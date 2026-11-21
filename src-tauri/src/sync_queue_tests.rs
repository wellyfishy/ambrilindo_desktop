//! The live sync sender against a small fake web (no Django needed): strict FIFO per event,
//! a 403 `event_not_granted` pausing only its event, and force sync superseding old rows.

use crate::sync::{test_auth, SyncService};
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};

/// Credentials and pauses are process-wide: one of these tests at a time.
static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Answers by the payload: `DENY` -> 403 event_not_granted, `NOTFOUND` -> 404, else 200.
/// Records every body it received, in order.
async fn fake_web() -> (String, Arc<Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let body = loop {
                    let Ok(n) = sock.read(&mut chunk).await else { return };
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let len = text[..end]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + len {
                            break String::from_utf8_lossy(&buf[end + 4..end + 4 + len]).to_string();
                        }
                    }
                };
                if body.contains("SLOW") {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                let (status, reply) = if body.contains("DENY") {
                    ("403 Forbidden", r#"{"error":"Event ini belum diizinkan untuk lisensi Anda.","code":"event_not_granted"}"#)
                } else if body.contains("NOTFOUND") {
                    ("404 Not Found", r#"{"error":"Partai tidak ditemukan"}"#)
                } else {
                    ("200 OK", r#"{"status":"success"}"#)
                };
                log.lock().unwrap().push(body);
                let resp = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    (url, seen)
}

/// Two events with live sync on (all others off) and an empty queue; returns their ids.
async fn two_events(pool: &SqlitePool) -> (i64, i64) {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM backend_event ORDER BY id LIMIT 2").fetch_all(pool).await.unwrap();
    assert_eq!(ids.len(), 2, "the DB copy needs two events");
    sqlx::query("DELETE FROM backend_syncqueue").execute(pool).await.unwrap();
    sqlx::query("UPDATE backend_event SET is_live_sync_enabled = 0").execute(pool).await.unwrap();
    sqlx::query("UPDATE backend_event SET is_live_sync_enabled = 1 WHERE id IN (?, ?)")
        .bind(ids[0])
        .bind(ids[1])
        .execute(pool)
        .await
        .unwrap();
    (ids[0], ids[1])
}

async fn enqueue(pool: &SqlitePool, event: i64, endpoint: &str, payload: &str) -> i64 {
    sqlx::query(
        "INSERT INTO backend_syncqueue (event_id, endpoint, payload, status, retry_count, created_at, updated_at) \
         VALUES (?, ?, ?, 'pending', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(event)
    .bind(endpoint)
    .bind(payload)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn status_of(pool: &SqlitePool, id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM backend_syncqueue WHERE id = ?").bind(id).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn event_not_granted_pauses_only_that_event() {
    let _g = LOCK.lock().await;
    let Some(t) = crate::test_support::real_db_copy().await else { return };
    let pool = t.db.get_pool_cloned().unwrap();
    let path = Some(&t.path);
    let (web, seen) = fake_web().await;
    SyncService::set_sync_config(path, "custom", Some(web)).unwrap();
    test_auth::set(Some(test_auth::Mode::Legacy("test".into())));
    let (a, b) = two_events(&pool).await;

    let a1 = enqueue(&pool, a, "api/status/", r#"{"k":"DENY-a1"}"#).await;
    let a2 = enqueue(&pool, a, "api/status/", r#"{"k":"a2"}"#).await;
    let b1 = enqueue(&pool, b, "api/status/", r#"{"k":"b1"}"#).await;
    let b2 = enqueue(&pool, b, "api/result/", r#"{"k":"b2"}"#).await;

    SyncService::process_sync_queue(&pool, path).await.unwrap();
    assert_eq!(status_of(&pool, a1).await, "failed", "refused head");
    assert_eq!(status_of(&pool, a2).await, "pending", "the refused event waits behind its head");
    assert_eq!(status_of(&pool, b1).await, "synced", "the other event is not held");
    assert_eq!(status_of(&pool, b2).await, "synced");
    assert!(SyncService::auth_status().blocked_reason.is_none(), "no global pause for one event");
    assert_eq!(SyncService::active_event_blocks().iter().map(|x| x.0).collect::<Vec<_>>(), vec![a], "event paused");

    // While paused the event is not tried again; new rows of the other event still go.
    let b3 = enqueue(&pool, b, "api/status/", r#"{"k":"b3"}"#).await;
    let before = seen.lock().unwrap().len();
    SyncService::process_sync_queue(&pool, path).await.unwrap();
    assert_eq!(status_of(&pool, b3).await, "synced");
    assert_eq!(seen.lock().unwrap().len(), before + 1, "only b3 was sent, the paused event was not retried");

    let heads = SyncService::get_queue_heads(&pool).await.unwrap();
    assert_eq!(heads.len(), 1, "{heads:?}");
    assert_eq!((heads[0].event_id, heads[0].item_id, heads[0].waiting), (Some(a), a1, 2));
    assert!(heads[0].paused_until.is_some(), "the page can show the pause");

    // "Kirim sekarang" clears the pause and tries the head again.
    SyncService::send_now(&pool, path).await.unwrap();
    assert_eq!(seen.lock().unwrap().iter().filter(|x| x.contains("DENY-a1")).count(), 2, "retried after Kirim sekarang");
    test_auth::set(None);
}

#[tokio::test]
async fn refused_head_holds_only_its_event() {
    let _g = LOCK.lock().await;
    let Some(t) = crate::test_support::real_db_copy().await else { return };
    let pool = t.db.get_pool_cloned().unwrap();
    let path = Some(&t.path);
    let (web, seen) = fake_web().await;
    SyncService::set_sync_config(path, "custom", Some(web)).unwrap();
    test_auth::set(Some(test_auth::Mode::Legacy("test".into())));
    let (a, b) = two_events(&pool).await;

    let a1 = enqueue(&pool, a, "api/result/", r#"{"k":"NOTFOUND-a1"}"#).await;
    let a2 = enqueue(&pool, a, "api/result/", r#"{"k":"a2"}"#).await;
    let b1 = enqueue(&pool, b, "api/result/", r#"{"k":"b1"}"#).await;

    let (synced, failed) = SyncService::process_sync_queue(&pool, path).await.unwrap();
    assert_eq!((synced, failed), (1, 1));
    assert_eq!(status_of(&pool, a1).await, "failed");
    assert_eq!(status_of(&pool, a2).await, "pending", "strict FIFO inside the event");
    assert_eq!(status_of(&pool, b1).await, "synced");
    assert!(SyncService::active_event_blocks().is_empty(), "a 404 is not a pause");
    assert!(!seen.lock().unwrap().iter().any(|x| x.contains("\"a2\"")), "nothing of the held event jumped ahead");
    test_auth::set(None);
}

/// A row saved for a healthy event while a pass is busy with another event's refused head is
/// sent by that same run (the rerun), not left for the worker's back-off.
#[tokio::test]
async fn row_saved_during_a_pass_is_sent_despite_another_held_event() {
    let _g = LOCK.lock().await;
    let Some(t) = crate::test_support::real_db_copy().await else { return };
    let pool = t.db.get_pool_cloned().unwrap();
    let path = t.path.clone();
    let (web, _seen) = fake_web().await;
    SyncService::set_sync_config(Some(&path), "custom", Some(web)).unwrap();
    test_auth::set(Some(test_auth::Mode::Legacy("test".into())));
    let (a, b) = two_events(&pool).await;

    let a1 = enqueue(&pool, a, "api/result/", r#"{"k":"NOTFOUND-SLOW-a1"}"#).await;
    let first = {
        let (pool, path) = (pool.clone(), path.clone());
        tokio::spawn(async move { SyncService::process_sync_queue(&pool, Some(&path)).await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(150)).await; // the pass is waiting on a1
    let b1 = enqueue(&pool, b, "api/result/", r#"{"k":"b1"}"#).await;
    // A save kicks a send; the run in progress takes it as "look again".
    assert_eq!(SyncService::process_sync_queue(&pool, Some(&path)).await.unwrap(), (0, 0), "another run is busy");
    first.await.unwrap().unwrap();
    assert_eq!(status_of(&pool, a1).await, "failed");
    assert_eq!(status_of(&pool, b1).await, "synced", "the rerun sent the healthy event's new row");
    test_auth::set(None);
}

#[tokio::test]
async fn force_sync_supersedes_old_rows_but_keeps_latest_tatami_status() {
    let _g = LOCK.lock().await;
    let Some(t) = crate::test_support::real_db_copy().await else { return };
    let pool = t.db.get_pool_cloned().unwrap();
    let (a, b) = two_events(&pool).await;

    let s1 = enqueue(&pool, a, "api/status/", r#"{"k":1}"#).await;
    let r1 = enqueue(&pool, a, "api/result/", r#"{"k":2}"#).await;
    let f1 = enqueue(&pool, a, "api/final-result/", r#"{"k":3}"#).await;
    let t1_old = enqueue(&pool, a, "api/sync/tatami-status/", r#"{"tatami_number":1,"is_active":true}"#).await;
    let t2 = enqueue(&pool, a, "api/sync/tatami-status/", r#"{"tatami_number":2,"is_active":true}"#).await;
    let t1_new = enqueue(&pool, a, "api/sync/tatami-status/", r#"{"tatami_number":1,"is_active":false}"#).await;
    let busy = enqueue(&pool, a, "api/status/", r#"{"k":4}"#).await;
    sqlx::query("UPDATE backend_syncqueue SET status = 'processing' WHERE id = ?").bind(busy).execute(&pool).await.unwrap();
    sqlx::query("UPDATE backend_syncqueue SET status = 'failed' WHERE id = ?").bind(s1).execute(&pool).await.unwrap();
    let other = enqueue(&pool, b, "api/status/", r#"{"k":5}"#).await;
    let max_id = other;
    let after = enqueue(&pool, a, "api/result/", r#"{"k":6}"#).await;

    let n = SyncService::supersede_queued_rows(&pool, a, max_id).await.unwrap();
    assert_eq!(n, 4, "status, result, final-result and the older tatami 1 status");
    for id in [s1, r1, f1, t1_old] {
        assert_eq!(status_of(&pool, id).await, "superseded", "row {id}");
    }
    for id in [t2, t1_new] {
        assert_eq!(status_of(&pool, id).await, "pending", "newest tatami status per tatami is kept: {id}");
    }
    assert_eq!(status_of(&pool, busy).await, "processing", "a row being sent is left alone");
    assert_eq!(status_of(&pool, other).await, "pending", "other events untouched");
    assert_eq!(status_of(&pool, after).await, "pending", "rows queued after the push started are still sent");

    let stats = SyncService::get_queue_stats(&pool, Some(a)).await.unwrap();
    assert_eq!((stats.pending_count, stats.failed_count, stats.synced_count), (4, 0, 4), "{stats:?}");
}

/// Generating a referchange queues its category's push (bracket before its results, retried
/// while offline); the new bagan gets its own kode, so no other bagan's kode moves; live sync
/// off queues nothing.
#[tokio::test]
async fn generated_referchange_is_queued_for_the_web() {
    use crate::qa_tests::{category_bagans, is_referchange, make_category, play_category};
    use serde_json::{json, Value};
    let _g = LOCK.lock().await;
    let Some(s) = crate::test_support::server_on_real_copy().await else { return };
    let pool = s.pool().await;
    let db_path = s.dir.join("db.sqlite3");
    const EV: i64 = 17;
    // Never production: an unreachable local target, and nothing is sent by itself.
    SyncService::set_sync_config(Some(&db_path), "custom", Some("http://127.0.0.1:9".into())).unwrap();
    SyncService::set_auto_send(Some(&db_path), false).unwrap();
    SyncService::set_event_mapping(Some(&db_path), EV, 999, "Uji", "custom").unwrap();
    sqlx::query("UPDATE backend_event SET is_live_sync_enabled = 1 WHERE id = ?").bind(EV).execute(&pool).await.unwrap();

    let cat = make_category(&s, EV, "QA KIRIM BABAK TAMBAHAN", 16).await;
    s.ok("execute_drawing_bagan", json!({ "eventId": EV, "categoryIds": [cat], "shuffleType": "random" })).await;
    play_category(&s, EV, cat).await;
    let kodes = || async {
        sqlx::query_as::<_, (i64, Option<String>)>("SELECT id, kode FROM backend_bagan WHERE event_id = ? ORDER BY id")
            .bind(EV).fetch_all(&pool).await.unwrap()
    };
    let main_id = category_bagans(&s, EV, cat).await[0]["id"].as_i64().unwrap();
    let push_rows = || async {
        sqlx::query_scalar::<_, String>("SELECT payload FROM backend_syncqueue WHERE event_id = ? AND endpoint = 'api/sync/push-bagan/' ORDER BY id")
            .bind(EV).fetch_all(&pool).await.unwrap()
    };
    let before_rows = push_rows().await.len();

    let res = s.ok("generate_repechage_bracket", json!({ "mainBaganId": main_id })).await;
    assert!(res.to_string().contains("antrean"), "operator is told: {res}");
    let rep = category_bagans(&s, EV, cat).await.into_iter().find(is_referchange).expect("referchange");
    let rep_kode = rep["kode"].as_str().unwrap_or("").to_string();
    assert!(!rep_kode.is_empty(), "referchange has its own kode");
    let rows = push_rows().await;
    assert_eq!(rows.len(), before_rows + 1, "one push queued");
    let p: Value = serde_json::from_str(rows.last().unwrap()).unwrap();
    assert_eq!(p["event_pk"], 999);
    let sent: Vec<&str> = p["bagans"].as_array().unwrap().iter().filter_map(|b| b["kode"].as_str()).collect();
    assert!(sent.contains(&rep_kode.as_str()), "referchange in the push: {sent:?}");
    assert_eq!(p["replace_bouts"], json!([rep_kode]), "only the referchange's web bouts are replaced");

    // Delete + generate again (a kode gap now exists): no other bagan's kode moves.
    let others_before: Vec<_> = kodes().await.into_iter().filter(|(id, _)| *id != rep["id"].as_i64().unwrap()).collect();
    s.ok("delete_repechage_bagan", json!({ "baganId": rep["id"], "force": true })).await;
    assert_eq!(push_rows().await.len(), before_rows + 2, "delete queues the category so the web drops it");
    s.ok("generate_repechage_bracket", json!({ "mainBaganId": main_id })).await;
    let after = kodes().await;
    for (id, k) in &others_before {
        assert_eq!(after.iter().find(|(i, _)| i == id).map(|(_, k)| k.clone()), Some(k.clone()), "kode of bagan {id} unchanged");
    }

    // Live sync off: generate ulang queues nothing.
    sqlx::query("UPDATE backend_event SET is_live_sync_enabled = 0 WHERE id = ?").bind(EV).execute(&pool).await.unwrap();
    let n = push_rows().await.len();
    let res = s.ok("generate_repechage_bracket", json!({ "mainBaganId": main_id })).await;
    assert_eq!(push_rows().await.len(), n, "live sync off: nothing queued");
    assert!(res.to_string().contains("manual"), "operator told to send by hand: {res}");
}
