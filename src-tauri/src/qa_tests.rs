//! End-to-end assurance tests: the real Axum router (what LAN devices call) on a private
//! copy of the tournament database, scenario = PORPROV VIII KALTIM 2026 (event 17).
//! Run: `cargo test --lib qa_` from `src-tauri/`.

use crate::test_support::{server_on_real_copy, TestServer};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const PORPROV: i64 = 17;

fn ids(v: &Value) -> Vec<i64> {
    v.as_array().map(|a| a.iter().filter_map(|x| x["id"].as_i64()).collect()).unwrap_or_default()
}

async fn bagans(s: &TestServer, event: i64) -> Vec<Value> {
    s.ok("get_bagans", json!({ "eventId": event })).await.as_array().cloned().unwrap_or_default()
}

async fn details(s: &TestServer, bagan: i64) -> Vec<Value> {
    s.ok("get_detail_bagan", json!({ "baganId": bagan })).await.as_array().cloned().unwrap_or_default()
}

/// Draw every category of the event that is still undrawn; returns the drawn category ids.
async fn draw_remaining(s: &TestServer, event: i64) -> Vec<i64> {
    let eligible = s.ok("get_drawing_eligible_categories", json!({ "eventId": event })).await;
    let cats: Vec<i64> = eligible
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["bagan_count"].as_i64() == Some(0))
        .filter_map(|c| c["id"].as_i64())
        .collect();
    if !cats.is_empty() {
        s.ok("execute_drawing_bagan", json!({ "eventId": event, "categoryIds": cats, "shuffleType": "random" })).await;
    }
    cats
}

fn playable(m: &Value) -> bool {
    m["selesai"].as_bool() == Some(false) && m["atlet1_id"].is_i64() && m["atlet2_id"].is_i64()
}

/// Play one bout: Aka wins 3–1 (kata: 5 judges, Aka higher).
async fn play(s: &TestServer, bagan: &Value, m: &Value) -> (u16, Value) {
    let kata = bagan["tipe_tanding"].as_str() == Some("1");
    let mut input = json!({
        "match_id": m["id"], "bagan_id": bagan["id"],
        "score1": if kata { "40.5" } else { "3" }, "score2": if kata { "39.0" } else { "1" },
        "pemenang": "1", "vr1": false, "vr2": false, "hantei": false, "selesai": true,
    });
    if kata {
        input["kata1"] = json!("HEIAN SHODAN");
        input["kata2"] = json!("HEIAN NIDAN");
        input["juri_scores1"] = json!(r#"["8.1","8.1","8.1","8.1","8.1"]"#);
        input["juri_scores2"] = json!(r#"["7.8","7.8","7.8","7.8","7.8"]"#);
    }
    s.cmd("save_match_result", json!({ "input": input })).await
}

/// Plays every bagan of the event to the end. Returns failures as (bagan, match, error).
async fn play_all(s: &TestServer, event: i64) -> Vec<String> {
    let mut errors = Vec::new();
    for _ in 0..12 {
        let mut progressed = false;
        for b in bagans(s, event).await {
            let bid = b["id"].as_i64().unwrap();
            for m in details(s, bid).await.iter().filter(|m| playable(m)) {
                let (code, body) = play(s, &b, m).await;
                if code == 200 {
                    progressed = true;
                } else {
                    errors.push(format!("{} match {} r{}: {code} {body}", b["nama_bagan"], m["id"], m["round"]));
                }
            }
        }
        if !progressed {
            break;
        }
    }
    errors
}

#[tokio::test]
async fn qa_porprov_draw_and_play_everything() {
    let Some(s) = server_on_real_copy().await else { return };

    let drawn = draw_remaining(&s, PORPROV).await;
    let all = bagans(&s, PORPROV).await;
    let cats: Vec<Value> = s.ok("get_categories", json!({ "eventId": PORPROV })).await.as_array().cloned().unwrap();
    eprintln!("drawn now: {} categories, bagans total: {}, categories: {}", drawn.len(), all.len(), cats.len());
    let with_bagan: HashSet<i64> = all.iter().filter_map(|b| b["nomor_tanding_id"].as_i64()).collect();
    for c in &cats {
        assert!(with_bagan.contains(&c["id"].as_i64().unwrap()), "category without bagan after drawing: {}", c["nama_nomor_tanding"]);
    }

    // structure: nobody twice in the same round of a bagan
    for b in &all {
        let mut per_round: HashMap<i64, Vec<i64>> = HashMap::new();
        for m in details(&s, b["id"].as_i64().unwrap()).await {
            let r = m["round"].as_i64().unwrap_or(0);
            for k in ["atlet1_id", "atlet2_id"] {
                if let Some(a) = m[k].as_i64() {
                    per_round.entry(r).or_default().push(a);
                }
            }
        }
        for (r, list) in per_round {
            let uniq: HashSet<_> = list.iter().collect();
            assert_eq!(uniq.len(), list.len(), "{} round {r}: athlete twice {list:?}", b["nama_bagan"]);
        }
    }

    let errors = play_all(&s, PORPROV).await;
    assert!(errors.is_empty(), "matches that could not be saved:\n{}", errors.join("\n"));

    // every bagan finished, with sane podium from its own category
    let (mut golds, mut bronzes) = (0, 0);
    for b in bagans(&s, PORPROV).await {
        let name = b["nama_bagan"].clone();
        let left: Vec<_> = details(&s, b["id"].as_i64().unwrap()).await.into_iter().filter(playable).collect();
        assert!(left.is_empty(), "{name}: {} matches still open", left.len());
        let j1 = b["juara_1_id"].as_i64();
        let j2 = b["juara_2_id"].as_i64();
        assert!(j1.is_some(), "{name}: no juara 1 after all matches played");
        assert!(j2.is_some() && j1 != j2, "{name}: juara 2 missing or equal to juara 1");
        let roster = s.ok("get_category_athletes", json!({ "categoryId": b["nomor_tanding_id"] })).await;
        let roster: HashSet<i64> = ids(&roster).into_iter().collect();
        let placed: HashSet<i64> = details(&s, b["id"].as_i64().unwrap()).await.iter().flat_map(|m| [m["atlet1_id"].as_i64(), m["atlet2_id"].as_i64()]).flatten().collect();
        if placed.len() >= 3 {
            assert!(b["juara_3a_id"].is_i64(), "{name}: {} entries but no juara 3", placed.len());
        }
        eprintln!("{name}: 1={} 2={} 3a={} 3b={}", b["juara_1_id"], b["juara_2_id"], b["juara_3a_id"], b["juara_3b_id"]);
        for k in ["juara_1_id", "juara_2_id", "juara_3a_id", "juara_3b_id"] {
            if let Some(a) = b[k].as_i64() {
                assert!(roster.contains(&a), "{name}: {k}={a} is not an athlete of this category");
            }
        }
        golds += 1;
        bronzes += ["juara_3a_id", "juara_3b_id"].iter().filter(|k| b[**k].is_i64()).count() as i64;
    }

    let summary = s.ok("get_summary_juara", json!({ "eventId": PORPROV })).await;
    assert_eq!(summary["total_gold"].as_i64(), Some(golds), "medal tally gold vs finished bagans: {summary}");
    let per_contingent: i64 = summary["utusan_standings"].as_array().unwrap().iter().map(|r| r["gold"].as_i64().unwrap_or(0)).sum();
    assert_eq!(per_contingent, golds, "sum of contingent golds must equal total");
    assert_eq!(summary["total_silver"].as_i64(), Some(golds), "one silver per finished bagan: {summary}");
    assert_eq!(summary["total_bronze"].as_i64(), Some(bronzes), "bronzes vs podium 3a/3b: {summary}");
    let rows = summary["utusan_standings"].as_array().unwrap();
    for (k, total) in [("silver", golds), ("bronze", bronzes)] {
        let sum: i64 = rows.iter().map(|r| r[k].as_i64().unwrap_or(0)).sum();
        assert_eq!(sum, total, "sum of contingent {k} must equal total");
    }
    // ranking: gold first, then silver, then bronze
    for w in rows.windows(2) {
        let key = |r: &Value| (r["gold"].as_i64(), r["silver"].as_i64(), r["bronze"].as_i64());
        assert!(key(&w[0]) >= key(&w[1]), "standings out of order: {} before {}", w[0]["nama"], w[1]["nama"]);
    }
}

/// Every drawable entry (individuals, or the team entry for beregu; `MEMBER_*` rows are
/// team members and are never drawn) is in its bracket, and no two entries from the same
/// contingent meet in round 1 when the field makes that avoidable.
#[tokio::test]
async fn qa_porprov_every_entry_drawn_and_contingents_separated() {
    let Some(s) = server_on_real_copy().await else { return };
    draw_remaining(&s, PORPROV).await;
    let pool = s.pool().await;
    for b in bagans(&s, PORPROV).await {
        let name = b["nama_bagan"].as_str().unwrap_or("").to_string();
        let entries: Vec<(i64, Option<i64>)> = sqlx::query_as(
            "SELECT id, utusan_id FROM backend_atlet WHERE nomor_tanding_id = ? AND (additional_code IS NULL OR additional_code NOT LIKE 'MEMBER_%')",
        )
        .bind(b["nomor_tanding_id"].as_i64())
        .fetch_all(&pool)
        .await
        .unwrap();
        let utusan: HashMap<i64, Option<i64>> = entries.iter().cloned().collect();
        let matches = details(&s, b["id"].as_i64().unwrap()).await;
        let placed: HashSet<i64> = matches.iter().flat_map(|m| [m["atlet1_id"].as_i64(), m["atlet2_id"].as_i64()]).flatten().collect();
        let missing: Vec<_> = utusan.keys().filter(|a| !placed.contains(a)).collect();
        assert!(missing.is_empty(), "{name}: entries not in the bracket: {missing:?}");
        let strangers: Vec<_> = placed.iter().filter(|a| !utusan.contains_key(a)).collect();
        assert!(strangers.is_empty(), "{name}: bracket holds ids that are not entries of this category: {strangers:?}");

        // same-contingent clash in round 1 is avoidable when no contingent has more than half the field
        let mut per: HashMap<Option<i64>, usize> = HashMap::new();
        for u in utusan.values() {
            *per.entry(*u).or_default() += 1;
        }
        let biggest = per.iter().filter(|(k, _)| k.is_some()).map(|(_, n)| *n).max().unwrap_or(0);
        if biggest * 2 <= utusan.len() {
            let first = first_real_round(&matches);
            for m in matches.iter().filter(|m| m["round"].as_i64() == Some(first) && m["atlet1_id"].is_i64() && m["atlet2_id"].is_i64()) {
                if let (Some(a), Some(c)) = (m["atlet1_id"].as_i64(), m["atlet2_id"].as_i64()) {
                    let (ua, uc) = (utusan[&a], utusan[&c]);
                    assert!(ua.is_none() || ua != uc, "{name}: first-round match {} pairs two entries of contingent {ua:?}", m["id"]);
                }
            }
        }
    }
}

fn first_real_round(ms: &[Value]) -> i64 {
    ms.iter()
        .filter(|m| m["atlet1_id"].is_i64() || m["atlet2_id"].is_i64())
        .filter_map(|m| m["round"].as_i64())
        .min()
        .unwrap_or(1)
}

fn in_round(ms: &[Value], round: i64, atlet: i64) -> Option<Value> {
    ms.iter()
        .find(|m| m["round"].as_i64() == Some(round) && (m["atlet1_id"].as_i64() == Some(atlet) || m["atlet2_id"].as_i64() == Some(atlet)))
        .cloned()
}

async fn save(s: &TestServer, b: &Value, m: &Value, winner: &str, selesai: bool) -> (u16, Value) {
    s.cmd(
        "save_match_result",
        json!({ "input": {
            "match_id": m["id"], "bagan_id": b["id"], "score1": "2", "score2": "1",
            "pemenang": winner, "vr1": false, "vr2": false, "hantei": false, "selesai": selesai,
        }}),
    )
    .await
}

/// Operator mistakes at the table: wrong winner, correcting a result, editing or redrawing
/// a category that already has results, and the deliberate delete + redraw path.
#[tokio::test]
async fn qa_porprov_result_and_bracket_guards() {
    let Some(s) = server_on_real_copy().await else { return };
    draw_remaining(&s, PORPROV).await;
    let b = bagans(&s, PORPROV)
        .await
        .into_iter()
        .find(|b| b["nama_bagan"].as_str() == Some("KUMITE -60 KG PUTRA"))
        .expect("-60 kg putra bagan");
    let bid = b["id"].as_i64().unwrap();
    let cat = b["nomor_tanding_id"].as_i64().unwrap();
    let ms = details(&s, bid).await;
    let r1 = first_real_round(&ms);
    let m1 = ms.iter().find(|m| m["round"].as_i64() == Some(r1) && playable(m)).cloned().expect("a real first-round bout");
    let (aka, ao) = (m1["atlet1_id"].as_i64().unwrap(), m1["atlet2_id"].as_i64().unwrap());

    // 1. a winner that is neither AKA nor AO is refused, nothing is saved
    let (code, body) = save(&s, &b, &m1, "9", true).await;
    assert_ne!(code, 200, "winner '9' must be refused: {body}");
    let (code, _) = save(&s, &b, &m1, "1", true).await;
    assert_eq!(code, 200);
    let ms = details(&s, bid).await;
    assert!(in_round(&ms, r1 + 1, aka).is_some(), "AKA winner must advance to the next round");

    // 2. correcting the result before the next bout is played moves the right athlete on
    let (code, body) = save(&s, &b, &m1, "2", true).await;
    assert_eq!(code, 200, "correction before next bout must be allowed: {body}");
    let ms = details(&s, bid).await;
    assert!(in_round(&ms, r1 + 1, aka).is_none(), "old winner must be removed from the next round after correction");
    let next = in_round(&ms, r1 + 1, ao).expect("corrected winner must be in the next round");

    // 3. once the next bout is finished, the earlier result is locked
    if playable(&next) {
        let (code, _) = save(&s, &b, &next, "1", true).await;
        assert_eq!(code, 200);
        let (code, body) = save(&s, &b, &m1, "1", true).await;
        assert_ne!(code, 200, "changing a result already used by a finished later bout must be refused: {body}");
    }

    // 4. redraw and manual edit of a category with results are refused
    let (code, body) = s.cmd("execute_drawing_bagan", json!({ "eventId": PORPROV, "categoryIds": [cat] })).await;
    assert_ne!(code, 200, "redraw with results must be refused: {body}");
    let (code, body) = s
        .cmd("update_bagan_details", json!({ "baganId": bid, "matchups": [{ "match_id": m1["id"], "atlet1_id": ao, "atlet2_id": aka }] }))
        .await;
    assert_ne!(code, 200, "swapping athletes of a finished bout must be refused: {body}");
    let (code, body) = s.cmd("update_bagan_details", json!({ "baganId": bid, "matchups": [{ "match_id": "x" }] })).await;
    assert_eq!(code, 400, "malformed bracket edit must be a 400, not a silent success: {body}");

    // 5. delete needs force when results exist; force + redraw works without FK errors
    let (code, body) = s.cmd("delete_category_bagans", json!({ "eventId": PORPROV, "categoryId": cat })).await;
    assert_ne!(code, 200, "deleting a bagan with results needs force: {body}");
    s.ok("delete_category_bagans", json!({ "eventId": PORPROV, "categoryId": cat, "force": true })).await;
    assert!(bagans(&s, PORPROV).await.iter().all(|x| x["nomor_tanding_id"].as_i64() != Some(cat)));
    s.ok("execute_drawing_bagan", json!({ "eventId": PORPROV, "categoryIds": [cat] })).await;
    let again = bagans(&s, PORPROV).await.into_iter().find(|x| x["nomor_tanding_id"].as_i64() == Some(cat)).expect("redrawn");
    assert!(details(&s, again["id"].as_i64().unwrap()).await.iter().all(|m| m["selesai"].as_bool() == Some(false) || !playable(m)));
}

// ---- realtime (WebSocket) -------------------------------------------------------

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMsg};

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn ws(s: &TestServer, channel: &str, tatami: i64) -> Ws {
    connect_async(format!("{}/ws/{channel}/{tatami}", s.ws_base)).await.expect("ws connect").0
}

async fn send(w: &mut Ws, v: Value) {
    w.send(WsMsg::Text(v.to_string().into())).await.unwrap();
}

/// Next JSON message of the given type within `ms`, skipping others; None on timeout.
async fn recv_type(w: &mut Ws, ty: &str, ms: u64) -> Option<Value> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, w.next()).await {
            Ok(Some(Ok(WsMsg::Text(t)))) => {
                let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                if v["type"] == ty {
                    return Some(v);
                }
            }
            Ok(Some(Ok(_))) => continue,
            _ => return None,
        }
    }
}

#[tokio::test]
async fn qa_realtime_tatami_relay_and_bracket_broadcast() {
    let Some(s) = server_on_real_copy().await else { return };
    let mut panel = ws(&s, "admin-control", 1).await;
    let mut board = ws(&s, "scoring-board", 1).await;
    let mut other_tatami = ws(&s, "scoring-board", 2).await;
    let mut overview = ws(&s, "bracket", 0).await;

    // an idle tatami still answers (tablets treat silence as a dead link)
    send(&mut board, json!({ "type": "request_tatami_state", "tatami": 9 })).await;
    let r = recv_type(&mut board, "tatami_state_none", 2000).await;
    assert!(r.is_some(), "idle tatami must answer request_tatami_state");

    // live score from the control panel reaches its tatami and the overview, not tatami 2,
    // and is not echoed back to the sender
    let live = json!({ "type": "tatami_live_state", "payload": { "tatami": 1, "score_aka": 3, "score_ao": 1 } });
    send(&mut panel, live).await;
    let got = recv_type(&mut board, "tatami_live_state", 2000).await.expect("scoreboard gets live state");
    assert_eq!(got["payload"]["score_aka"], 3);
    assert!(recv_type(&mut overview, "tatami_live_state", 2000).await.is_some(), "overview gets every tatami");
    assert!(recv_type(&mut other_tatami, "tatami_live_state", 400).await.is_none(), "tatami 2 must not get tatami 1 traffic");
    assert!(recv_type(&mut panel, "tatami_live_state", 400).await.is_none(), "sender must not get its own message back");

    // a tablet that (re)connects later is served the cached live state
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let mut late = ws(&s, "jury-panel", 1).await;
    send(&mut late, json!({ "type": "request_tatami_state", "tatami": 1 })).await;
    let cached = recv_type(&mut late, "tatami_live_state", 2000).await.expect("late tablet gets cached state");
    assert_eq!(cached["payload"]["score_aka"], 3);

    // a bracket change made over HTTP reaches all devices
    draw_remaining(&s, PORPROV).await;
    let b = bagans(&s, PORPROV).await.into_iter().find(|b| b["nama_bagan"].as_str() == Some("KUMITE -84 KG PUTRA")).unwrap();
    s.ok("update_bagan_details", json!({ "baganId": b["id"], "namaBagan": "KUMITE -84 KG PUTRA", "matchups": [] })).await;
    for (name, w) in [("tatami 1", &mut board), ("tatami 2", &mut other_tatami), ("overview", &mut overview)] {
        assert!(recv_type(w, "bracket_updated", 2000).await.is_some(), "{name} must get bracket_updated");
    }
}

// ---- kumite beregu (team) --------------------------------------------------------

#[tokio::test]
async fn qa_porprov_team_kumite_bouts() {
    let Some(s) = server_on_real_copy().await else { return };
    draw_remaining(&s, PORPROV).await;
    let b = bagans(&s, PORPROV).await.into_iter().find(|b| b["nama_bagan"].as_str() == Some("KUMITE BEREGU PUTRA")).unwrap();
    let bid = b["id"].as_i64().unwrap();
    let cat = b["nomor_tanding_id"].as_i64().unwrap();
    let ms = details(&s, bid).await;
    let r1 = first_real_round(&ms);
    let parent = ms.iter().find(|m| m["round"].as_i64() == Some(r1) && playable(m)).cloned().expect("a team bout");
    let pid = parent["id"].as_i64().unwrap();

    let info = s.ok("get_team_match_info", json!({ "matchId": pid })).await;
    assert_eq!(info["is_team_match"], true);
    let bouts = info["bouts"].as_array().unwrap().clone();
    assert_eq!(bouts.len(), 5, "putra team kumite has 5 bouts by default: {}", bouts.len());
    let aka_c: Vec<i64> = ids(&info["candidate_atlets_aka"]);
    let ao_c: Vec<i64> = ids(&info["candidate_atlets_ao"]);
    assert!(aka_c.len() >= 3 && ao_c.len() >= 3, "each team offers its members: aka {aka_c:?} ao {ao_c:?}");

    // opening the match again must not create a second set of bouts
    let again = s.ok("get_team_match_info", json!({ "matchId": pid })).await;
    assert_eq!(again["bouts"].as_array().unwrap().len(), 5, "re-opening a team match duplicated its bouts");

    // lineup: three bouts with members, one with a custom (typed) name
    for (i, bt) in bouts.iter().take(3).enumerate() {
        s.ok("assign_bout_athletes", json!({ "input": { "bout_id": bt["bout_id"], "aka_atlet_id": aka_c[i], "ao_atlet_id": ao_c[i] } })).await;
    }
    let entries_before: i64 = s.ok("get_drawing_eligible_categories", json!({ "eventId": PORPROV })).await
        .as_array().unwrap().iter().find(|c| c["id"].as_i64() == Some(cat)).unwrap()["athlete_count"].as_i64().unwrap();
    s.ok("assign_bout_athletes", json!({ "input": { "bout_id": bouts[3]["bout_id"], "aka_custom_name": "Pengganti Darurat", "ao_atlet_id": ao_c[0] } })).await;
    let cats = s.ok("get_drawing_eligible_categories", json!({ "eventId": PORPROV })).await;
    let c = cats.as_array().unwrap().iter().find(|c| c["id"].as_i64() == Some(cat)).unwrap().clone();
    assert_eq!(c["athlete_count"].as_i64(), Some(entries_before), "a typed substitute must not become a new team entry: {c}");
    assert_eq!(c["athletes_not_in_bagan"].as_i64(), Some(0), "a typed substitute must not show as 'not in bagan': {c}");

    // bouts: AKA wins 2, AO wins 1 -> AKA team wins and advances
    for (bt, w) in bouts.iter().take(3).zip(["1", "1", "2"]) {
        let (code, body) = s.cmd("save_match_result", json!({ "input": {
            "match_id": bt["bout_id"], "bagan_id": bid, "score1": if w == "1" { "3" } else { "0" }, "score2": if w == "2" { "2" } else { "1" },
            "pemenang": w, "vr1": false, "vr2": false, "hantei": false, "selesai": true }})).await;
        assert_eq!(code, 200, "bout save: {body}");
    }
    let info = s.ok("get_team_match_info", json!({ "matchId": pid })).await;
    assert_eq!((info["team_aka_score"].as_i64(), info["team_ao_score"].as_i64()), (Some(2), Some(1)));
    s.ok("finish_team_match", json!({ "parentMatchId": pid })).await;
    let ms = details(&s, bid).await;
    let p = ms.iter().find(|m| m["id"].as_i64() == Some(pid)).unwrap();
    assert_eq!(p["pemenang"], "1");
    assert!(in_round(&ms, r1 + 1, parent["atlet1_id"].as_i64().unwrap()).is_some(), "winning team must advance");

    // a full tie is never handed to AKA silently
    let next = ms.iter().find(|m| m["round"].as_i64() == Some(r1) && playable(m) && m["id"].as_i64() != Some(pid)).cloned().expect("second team bout");
    let nid = next["id"].as_i64().unwrap();
    let nb = s.ok("get_team_match_info", json!({ "matchId": nid })).await["bouts"].as_array().unwrap().clone();
    for (bt, w) in nb.iter().take(2).zip(["1", "2"]) {
        s.cmd("save_match_result", json!({ "input": { "match_id": bt["bout_id"], "bagan_id": bid, "score1": "1", "score2": "1",
            "pemenang": w, "vr1": false, "vr2": false, "hantei": false, "selesai": true }})).await;
    }
    let (code, body) = s.cmd("finish_team_match", json!({ "parentMatchId": nid })).await;
    assert_ne!(code, 200, "1-1 with equal points must ask for a deciding bout: {body}");
    let extra = s.ok("add_team_bout", json!({ "parentMatchId": nid })).await;
    assert!(extra["bout_id"].as_i64().is_some(), "deciding bout can be added");
    s.ok("finish_team_match", json!({ "parentMatchId": nid, "winnerOverride": "2" })).await;
}

// ---- referee panel & chief referee approval -----------------------------------------

#[tokio::test]
async fn qa_porprov_bout_panel_approval_flow() {
    let Some(s) = server_on_real_copy().await else { return };
    draw_remaining(&s, PORPROV).await;
    let pool = s.pool().await;
    let wasits: Vec<i64> = sqlx::query_scalar("SELECT id FROM backend_wasit WHERE event_id = ? ORDER BY id LIMIT 6")
        .bind(PORPROV).fetch_all(&pool).await.unwrap();
    assert!(wasits.len() >= 6, "PORPROV has referees");
    let b = bagans(&s, PORPROV).await.into_iter().find(|b| b["nama_bagan"].as_str() == Some("KUMITE -75 KG PUTRA")).unwrap();
    let ms = details(&s, b["id"].as_i64().unwrap()).await;
    let m = ms.iter().find(|m| playable(m)).cloned().unwrap();
    let mid = m["id"].as_i64().unwrap();
    let seats = json!([
        { "posisi": "referee", "wasit_id": wasits[0] }, { "posisi": "judge_1", "wasit_id": wasits[1] },
        { "posisi": "judge_2", "wasit_id": wasits[2] }, { "posisi": "judge_3", "wasit_id": wasits[3] },
        { "posisi": "judge_4", "wasit_id": wasits[4] },
    ]);

    s.ok("set_event_referee_settings", json!({ "eventId": PORPROV, "requireChiefApproval": true })).await;

    // a draft (not submitted) bout cannot be sent for approval
    let st = s.ok("request_bout_approval", json!({ "matchId": mid, "tatamiNumber": 1 })).await;
    assert_eq!(st["required"], true);
    assert_ne!(st["status"], "pending", "an unsubmitted panel must not reach the chief referee: {st}");

    // two tablets: the second save with the old version is refused
    let rev0 = s.ok("get_bout_panel", json!({ "matchId": mid, "tatamiNumber": 1 })).await["rev"].as_i64();
    s.ok("save_bout_panel", json!({ "matchId": mid, "tatamiNumber": 1, "seats": seats, "expectedRev": rev0 })).await;
    let (code, body) = s.cmd("save_bout_panel", json!({ "matchId": mid, "tatamiNumber": 1, "seats": seats, "expectedRev": rev0 })).await;
    assert_ne!(code, 200, "stale panel save from a second tablet must be refused: {body}");

    // submit -> request -> pending -> reject (back to draft with the note)
    let rev = s.ok("get_bout_panel", json!({ "matchId": mid, "tatamiNumber": 1 })).await["rev"].as_i64();
    s.ok("submit_bout_panel", json!({ "matchId": mid, "tatamiNumber": 1, "seats": seats, "expectedRev": rev })).await;
    let st = s.ok("request_bout_approval", json!({ "matchId": mid, "tatamiNumber": 1 })).await;
    assert_eq!(st["status"], "pending");
    let queue = s.ok("get_approval_queue", json!({ "eventId": PORPROV })).await.to_string();
    assert!(queue.contains(&format!("{mid}")), "pending bout must be in the chief referee queue");
    s.ok("decide_bout_approval", json!({ "matchId": mid, "approve": false, "note": "Juri 2 satu daerah dengan AKA" })).await;
    let st = s.ok("request_bout_approval", json!({ "matchId": mid, "tatamiNumber": 1 })).await;
    assert_eq!(st["status"], "draft");
    assert_eq!(st["reject_note"], "Juri 2 satu daerah dengan AKA");

    // deciding again on a bout that is no longer pending is refused
    let (code, _) = s.cmd("decide_bout_approval", json!({ "matchId": mid, "approve": true })).await;
    assert_ne!(code, 200, "approve of a non-pending bout must be refused");

    // resubmit -> approve
    let rev = s.ok("get_bout_panel", json!({ "matchId": mid, "tatamiNumber": 1 })).await["rev"].as_i64();
    s.ok("submit_bout_panel", json!({ "matchId": mid, "tatamiNumber": 1, "seats": seats, "expectedRev": rev })).await;
    s.ok("request_bout_approval", json!({ "matchId": mid, "tatamiNumber": 1 })).await;
    s.ok("decide_bout_approval", json!({ "matchId": mid, "approve": true })).await;
    let st = s.ok("request_bout_approval", json!({ "matchId": mid, "tatamiNumber": 1 })).await;
    assert_eq!(st["status"], "approved");

    // the approved officials are busy for other tatami while the bout is unfinished
    let busy = s.ok("get_referee_busy", json!({ "eventId": PORPROV })).await.to_string();
    assert!(busy.contains(&format!("{}", wasits[0])), "referee of an approved, unfinished bout must show busy: {busy:.300}");
}

// ---- results, booklet, Excel -------------------------------------------------------

#[tokio::test]
async fn qa_porprov_reports_after_full_tournament() {
    let Some(s) = server_on_real_copy().await else { return };
    draw_remaining(&s, PORPROV).await;
    assert!(play_all(&s, PORPROV).await.is_empty());
    let all = bagans(&s, PORPROV).await;
    let pool = s.pool().await;
    let name_of = |id: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT nama_atlet FROM backend_atlet WHERE id = ?").bind(id).fetch_one(&pool).await.unwrap()
        }
    };

    // booklet: every category, same totals as the medal summary, all bouts finished
    let book = s.ok("get_results_booklet_data", json!({ "eventId": PORPROV })).await;
    let summary = s.ok("get_summary_juara", json!({ "eventId": PORPROV })).await;
    for k in ["total_gold", "total_silver", "total_bronze"] {
        assert_eq!(book[k], summary[k], "booklet {k} differs from medal summary");
    }
    assert_eq!(book["category_winners"].as_array().unwrap().len(), all.len(), "booklet lists every category");
    let ba = &book["berita_acara"];
    assert_eq!(ba["total_partai"], ba["partai_selesai"], "berita acara: all bouts finished: {ba}");
    assert_eq!(ba["total_kontingen"].as_i64(), Some(10), "10 kab/kota");

    // Excel: opens, and lists every gold medallist
    let r = s.http.get(format!("{}/api/export-excel/{PORPROV}", s.base)).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 200);
    let bytes = r.bytes().await.unwrap().to_vec();
    use calamine::Reader;
    let mut wb: calamine::Xlsx<_> = calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).expect("valid xlsx");
    let sheets = wb.sheet_names().to_owned();
    assert!(sheets.len() >= 2, "multi-sheet workbook: {sheets:?}");
    let mut text = String::new();
    for sh in &sheets {
        if let Ok(range) = wb.worksheet_range(sh) {
            for row in range.rows() {
                for c in row {
                    text.push_str(&c.to_string());
                    text.push('|');
                }
            }
        }
    }
    let text = text.to_uppercase();
    for b in &all {
        let gold = name_of(b["juara_1_id"].as_i64().unwrap()).await.to_uppercase();
        assert!(text.contains(&gold), "Excel misses gold medallist {gold} of {}", b["nama_bagan"]);
    }

    // marshalling and per-tatami views still load after the event is done
    s.ok("get_marshalling_overview", json!({ "eventId": PORPROV })).await;
    s.ok("get_summary_bagan", json!({ "eventId": PORPROV })).await;
}

/// Manual / browser QA: the real router + the built frontend (`npm run build`) on
/// http://127.0.0.1:8010 (QA_PORT to change), backed by a fresh copy of the tournament
/// database. Never touches the real database.
///   cargo test --lib qa_serve_for_manual_testing -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn qa_serve_for_manual_testing() {
    let port = std::env::var("QA_PORT").unwrap_or_else(|_| "8010".into());
    let Some(s) = crate::test_support::server_on_real_copy_at(&format!("127.0.0.1:{port}")).await else { return };
    // logos and photos, copied next to the DB copy (read only from the real folder)
    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        let _ = std::fs::create_dir_all(to);
        for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &target);
            } else {
                let _ = std::fs::copy(e.path(), target);
            }
        }
    }
    copy_dir(std::path::Path::new("../tournament_data/media"), &s.dir.join("media"));
    eprintln!("QA server on {} (DB copy). Ctrl+C to stop.", s.base);
    tokio::time::sleep(std::time::Duration::from_secs(4 * 3600)).await;
}

#[tokio::test]
async fn qa_tatami_numbers_are_unique_per_event() {
    let Some(s) = server_on_real_copy().await else { return };
    // the real event may already have tatami: work above the highest number
    let a = free_tatami_number(&s, PORPROV).await;
    let (b, c) = (a + 1, a + 2);
    let t1 = s.ok("create_tatami", json!({ "eventId": PORPROV, "tatamiNumber": a })).await;
    let t2 = s.ok("create_tatami", json!({ "eventId": PORPROV, "tatamiNumber": b })).await;
    let (code, body) = s.cmd("create_tatami", json!({ "eventId": PORPROV, "tatamiNumber": a })).await;
    assert_ne!(code, 200, "a second tatami {a} in the same event must be refused: {body}");
    let (code, _) = s.cmd("create_tatami", json!({ "eventId": PORPROV, "tatamiNumber": 0 })).await;
    assert_ne!(code, 200, "tatami 0 must be refused");
    let (code, body) = s.cmd("update_tatami_number", json!({ "tatamiId": t2["id"], "newNumber": a })).await;
    assert_ne!(code, 200, "renumbering onto a taken number must be refused: {body}");
    s.ok("update_tatami_number", json!({ "tatamiId": t2["id"], "newNumber": c })).await;
    s.ok("update_tatami_number", json!({ "tatamiId": t1["id"], "newNumber": a })).await; // unchanged number is fine
    // other events may reuse the number
    let other = free_tatami_number(&s, 16).await.max(free_tatami_number(&s, PORPROV).await);
    s.ok("create_tatami", json!({ "eventId": 16, "tatamiNumber": other })).await;
    s.ok("create_tatami", json!({ "eventId": PORPROV, "tatamiNumber": other })).await;
}

/// Lowest tatami number above every tatami the event already has.
async fn free_tatami_number(s: &TestServer, event_id: i64) -> i64 {
    let (max,): (Option<i64>,) = sqlx::query_as("SELECT MAX(tatami_number) FROM backend_tatami WHERE event_id = ?")
        .bind(event_id)
        .fetch_one(&s.pool().await)
        .await
        .unwrap();
    max.unwrap_or(0) + 1
}

/// A tablet that connects before the control panel broadcasts (or after a host restart)
/// gets the live state built from the DB: a kata bout must say kata even before the LO
/// has chosen the kata names.
#[tokio::test]
async fn qa_live_state_from_db_knows_kata_before_kata_names() {
    let Some(s) = server_on_real_copy().await else { return };
    draw_remaining(&s, PORPROV).await;
    let t = free_tatami_number(&s, PORPROV).await;
    s.ok("create_tatami", json!({ "eventId": PORPROV, "tatamiNumber": t })).await;
    s.ok("create_tatami", json!({ "eventId": PORPROV, "tatamiNumber": t + 1 })).await;
    for (bagan, tatami, want) in [("KATA PERORANGAN PUTRI", t, true), ("KUMITE -61 KG PUTRI", t + 1, false)] {
        let b = bagans(&s, PORPROV).await.into_iter().find(|b| b["nama_bagan"].as_str() == Some(bagan)).unwrap();
        let m = details(&s, b["id"].as_i64().unwrap()).await.into_iter().find(|m| playable(m)).unwrap();
        assert!(m["kata1"].is_null(), "precondition: no kata chosen yet");
        s.ok("notify_match_running", json!({ "matchId": m["id"], "tatami": tatami })).await;
        let mut w = ws(&s, "jury-panel", tatami).await;
        send(&mut w, json!({ "type": "request_tatami_state", "tatami": tatami })).await;
        let st = recv_type(&mut w, "tatami_live_state", 2000).await.expect("live state");
        assert_eq!(st["payload"]["isKata"], want, "{bagan}: {st}");
        assert_eq!(st["payload"]["category"], bagan);
    }
}

/// The roster counts entries: for a team category that is the teams, not every member row.
#[tokio::test]
async fn qa_timetable_counts_teams_not_members() {
    let Some(t) = crate::test_support::real_db_copy().await else { return };
    let pool = t.pool();
    let teams: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT nomor_tanding_id, COUNT(*) FROM backend_atlet WHERE event_id = ? AND nomor_tanding_id IS NOT NULL
           AND (additional_code IS NULL OR additional_code NOT LIKE 'MEMBER_%') GROUP BY nomor_tanding_id",
    )
    .bind(PORPROV)
    .fetch_all(&pool)
    .await
    .unwrap();
    let teams: HashMap<i64, i64> = teams.into_iter().collect();
    let data = t.db.get_timetable_editor_data(PORPROV).await.expect("timetable");
    let mut checked_team_cat = false;
    for c in &data.categories {
        let want = teams.get(&c.id).copied().unwrap_or(0);
        assert_eq!(c.atlet_count, want, "{}: roster count must exclude MEMBER_ rows", c.nama);
        if c.nama.to_uppercase().contains("BEREGU") && want > 0 {
            checked_team_cat = true;
            assert_eq!(c.matches, want - 1, "{}: matches follow the team count", c.nama);
        }
    }
    assert!(checked_team_cat, "PORPROV must have a team category to check");
    for day in &data.days {
        for row in &day.rows {
            for cell in &row.cells {
                if let Some(id) = cell.nomor_tanding_id {
                    if !cell.is_festival {
                        assert_eq!(cell.atlet_count, teams.get(&id).copied().unwrap_or(0), "cell {:?}", cell.nomor_tanding_nama);
                    }
                }
            }
        }
    }
}

// ---- one category on two tatamis (pool atas / pool bawah / final) -------------------

#[test]
fn split_part_math() {
    use crate::db::{count_bouts_in_part, is_pool_split, missing_split_parts, pick_part_bagans, split_part_share};
    // 9 entries, 8 bouts, 40 minutes: halves 5 + 4, bouts 4 + 3 + final 1
    assert_eq!(split_part_share(9, 8, 40, "top"), (5, 4, 20));
    assert_eq!(split_part_share(9, 8, 40, "bottom"), (4, 3, 15));
    assert_eq!(split_part_share(9, 8, 40, "final"), (2, 1, 5));
    assert_eq!(split_part_share(9, 8, 40, "top_final"), (5, 5, 25));
    assert_eq!(split_part_share(9, 8, 40, "nonsense"), (9, 8, 40));

    assert!(missing_split_parts(&[]).is_empty(), "not scheduled at all is not 'missing parts'");
    assert!(missing_split_parts(&[None]).is_empty());
    assert_eq!(missing_split_parts(&[Some("top"), Some("bottom")]), vec!["final"]);
    assert!(missing_split_parts(&[Some("top_final"), Some("bottom")]).is_empty());
    assert_eq!(missing_split_parts(&[Some("bottom_final")]), vec!["top"]);

    // 16-slot quadrant bracket: rounds of 8, 4, 2, 1 (+ a round-5 bout)
    let mut bouts = vec![];
    for (r, n) in [(1, 8), (2, 4), (3, 2), (4, 1), (5, 1)] {
        for u in 1..=n {
            bouts.push((r, u));
        }
    }
    let (t, b, f) = (count_bouts_in_part(&bouts, "top"), count_bouts_in_part(&bouts, "bottom"), count_bouts_in_part(&bouts, "final"));
    assert_eq!((t, b, f), (7, 7, 2));
    assert_eq!(count_bouts_in_part(&bouts, "bottom_final"), 9);

    // Pool A/B + Final (+ repechage): each part prints its own bagans
    let pooled = vec![
        (10, "X - Final".to_string(), 0),
        (11, "X - Pool A".to_string(), 2),
        (12, "X - Pool B".to_string(), 2),
        (13, "X - REPECHAGE".to_string(), 0),
    ];
    assert!(is_pool_split(&pooled));
    assert_eq!(pick_part_bagans(&pooled, Some("top")), vec![11]);
    assert_eq!(pick_part_bagans(&pooled, Some("bottom_final")), vec![12, 10, 13]);
    assert_eq!(pick_part_bagans(&pooled, None), vec![10, 11, 12, 13]);
    // one bracket (a repechage bagan may also have pool 0): both halves print that bracket
    let single = vec![(20, "Y".to_string(), 1), (21, "Y - REPECHAGE".to_string(), 0)];
    assert!(!is_pool_split(&single));
    assert_eq!(pick_part_bagans(&single, Some("top")), vec![20]);
    assert_eq!(pick_part_bagans(&single, Some("final")), vec![20, 21]);
}

#[tokio::test]
async fn qa_split_category_over_two_tatamis() {
    use crate::models::{SaveTimetableCellInput, SaveTimetableDayInput, SaveTimetablePayload, SaveTimetableRowInput};
    let Some(t) = crate::test_support::real_db_copy().await else { return };
    let pool = t.pool();
    // a drawn single-bracket category with enough entries
    let (nt, name, bagan): (i64, String, i64) = sqlx::query_as(
        "SELECT n.id, n.nama_nomor_tanding, b.id FROM backend_nomortanding n JOIN backend_bagan b ON b.nomor_tanding_id = n.id
          WHERE n.event_id = ? AND n.nama_nomor_tanding = 'KATA PERORANGAN PUTRA' LIMIT 1",
    )
    .bind(PORPROV)
    .fetch_one(&pool)
    .await
    .expect("PORPROV kata perorangan putra is drawn");
    let mut tatamis: Vec<(i64,)> = sqlx::query_as("SELECT id FROM backend_tatami WHERE event_id = ? ORDER BY tatami_number").bind(PORPROV).fetch_all(&pool).await.unwrap();
    while tatamis.len() < 2 {
        let n = 100 + tatamis.len() as i32;
        tatamis.push((t.db.create_tatami(PORPROV, n).await.unwrap().id,));
    }
    let (t1, t2) = (tatamis[0].0, tatamis[1].0);
    let day = t.db.add_timetable_day(PORPROV).await.unwrap().id;
    // the real timetable may already schedule it (whole) on another day: start from none
    sqlx::query("DELETE FROM backend_timetablecell WHERE nomor_tanding_id = ?").bind(nt).execute(&pool).await.unwrap();

    let cell = |tatami: i64, part: Option<&str>| SaveTimetableCellInput {
        tatami_id: tatami,
        nomor_tanding_id: Some(nt),
        custom_text: String::new(),
        part: part.map(str::to_string),
    };
    let row = |cells: Vec<SaveTimetableCellInput>| SaveTimetableRowInput {
        id: None,
        order: 0,
        row_type: "slot".into(),
        time_label: String::new(),
        label_text: String::new(),
        cells,
    };
    let save = |rows: Vec<SaveTimetableRowInput>| SaveTimetablePayload { days: vec![SaveTimetableDayInput { day_id: day, tanggal: None, rows }] };

    // session 1: pool atas on T1, pool bawah on T2; session 2: final on T1
    t.db.save_timetable_data(PORPROV, save(vec![row(vec![cell(t1, Some("top")), cell(t2, Some("bottom"))]), row(vec![cell(t1, Some("final"))])]))
        .await
        .unwrap();

    let data = t.db.get_timetable_editor_data(PORPROV).await.unwrap();
    let whole = data.categories.iter().find(|c| c.id == nt).unwrap().clone();
    let d = data.days.iter().find(|d| d.id == day).unwrap();
    let s1 = &d.rows[0];
    assert!(s1.conflicts.is_empty(), "two halves of one category are not an athlete clash: {:?}", s1.conflicts);
    let top = s1.cells.iter().find(|c| c.tatami_id == t1).unwrap();
    let bottom = s1.cells.iter().find(|c| c.tatami_id == t2).unwrap();
    let fin = d.rows[1].cells.iter().find(|c| c.tatami_id == t1).unwrap();
    assert_eq!((top.part.as_deref(), bottom.part.as_deref(), fin.part.as_deref()), (Some("top"), Some("bottom"), Some("final")));
    assert_eq!(top.atlet_count + bottom.atlet_count, whole.atlet_count, "halves share the entries");
    assert_eq!(top.matches + bottom.matches + fin.matches, whole.matches, "parts share the bouts");
    assert!(top.est_duration_minutes < whole.est_duration_minutes);

    // Buku Tatami: T1 prints the pool atas and the final, T2 the pool bawah, each of this bracket
    let b1 = t.db.get_tatami_booklet_data(PORPROV, day, t1).await.unwrap();
    let b2 = t.db.get_tatami_booklet_data(PORPROV, day, t2).await.unwrap();
    let names1: Vec<&str> = b1.ordered_categories.iter().map(|c| c.category_name.as_str()).collect();
    assert_eq!(names1, vec![format!("{name} (POOL ATAS)"), format!("{name} (FINAL)")]);
    assert_eq!(b2.ordered_categories[0].category_name, format!("{name} (POOL BAWAH)"));
    for c in b1.ordered_categories.iter().chain(&b2.ordered_categories) {
        assert_eq!(c.bagan_id, Some(bagan));
    }
    let total = b1.ordered_categories[0].matches.len() as i64;
    let counted: i64 = b1.ordered_categories.iter().chain(&b2.ordered_categories).map(|c| c.matches_count).sum();
    assert_eq!(counted, total, "every bout of the bracket is on exactly one tatami");
    assert!(!b1.unscheduled_list.iter().any(|u| u.starts_with(&name)), "{:?}", b1.unscheduled_list);

    // final not placed yet: it is listed as unscheduled
    t.db.save_timetable_data(PORPROV, save(vec![row(vec![cell(t1, Some("top")), cell(t2, Some("bottom"))])])).await.unwrap();
    let b1 = t.db.get_tatami_booklet_data(PORPROV, day, t1).await.unwrap();
    assert!(b1.unscheduled_list.contains(&format!("{name} (FINAL)")), "{:?}", b1.unscheduled_list);

    // a part on a cell without a category, or an unknown part, is not stored
    t.db.save_timetable_data(PORPROV, save(vec![row(vec![
        SaveTimetableCellInput { tatami_id: t1, nomor_tanding_id: None, custom_text: "ISHOMA".into(), part: Some("top".into()) },
        cell(t2, Some("middle")),
    ])]))
    .await
    .unwrap();
    let parts: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT c.part FROM backend_timetablecell c JOIN backend_timetablerow r ON r.id = c.row_id WHERE r.day_id = ?",
    )
    .bind(day)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(parts.iter().all(|p| p.is_none()), "{parts:?}");
}
