use crate::models::{EnrichedAtlet, EnrichedWasitTatami, WasitConflict};

/// Normalizes location string (kab/kota, kontingen) by trimming, lowercasing,
/// and removing administrative noise words like "kota", "kabupaten", "kab.", "provinsi", "dki", "kontingen", "dojo", "team".
pub fn normalize_location(s: &str) -> String {
    let mut cleaned = s.to_lowercase();
    let noise_words = [
        "kabupaten", "kab.", "kab", "kota", "provinsi", "prov.", "prov",
        "dki", "daerah", "kontingen", "dojo", "team", "tim", "perguruan",
        "pengcab", "pengprov", "club", "karate"
    ];

    for noise in &noise_words {
        cleaned = cleaned.replace(noise, " ");
    }

    // Retain alphanumeric and spaces
    cleaned
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Checks whether two location strings match (either identical after normalization or sharing significant words).
pub fn is_location_conflict(wasit_loc: Option<&str>, athlete_loc: Option<&str>) -> bool {
    let (Some(w_raw), Some(a_raw)) = (wasit_loc, athlete_loc) else {
        return false;
    };

    let w = normalize_location(w_raw);
    let a = normalize_location(a_raw);

    if w.is_empty() || a.is_empty() {
        return false;
    }

    if w == a || w.contains(&a) || a.contains(&w) {
        return true;
    }

    // Check individual significant word tokens (min length 4)
    let w_tokens: Vec<&str> = w.split_whitespace().filter(|t| t.len() >= 4).collect();
    let a_tokens: Vec<&str> = a.split_whitespace().filter(|t| t.len() >= 4).collect();

    for wt in &w_tokens {
        for at in &a_tokens {
            if wt == at {
                return true;
            }
        }
    }

    false
}

/// Evaluates potential conflict between a referee assignment and an athlete competing on their Tatami.
pub fn check_wasit_athlete_conflict(
    wasit: &EnrichedWasitTatami,
    athlete: &EnrichedAtlet,
    side: &str, // "AKA" or "AO"
    match_id: Option<i64>,
    match_kode: Option<&str>,
) -> Option<WasitConflict> {
    let athlete_nama = athlete.nama_atlet.as_deref().unwrap_or("Atlet").to_string();

    // 1. Perguruan Conflict Check:
    // If referee and athlete belong to the exact same Perguruan
    if let (Some(w_perg_id), Some(a_perg_id)) = (wasit.perguruan_id, athlete.perguruan_id) {
        if w_perg_id > 0 && w_perg_id == a_perg_id {
            let perg_nama = wasit
                .nama_perguruan
                .clone()
                .or_else(|| athlete.perguruan_nama.clone())
                .unwrap_or_else(|| format!("Perguruan #{}", w_perg_id));

            return Some(WasitConflict {
                wasit_id: wasit.wasit_id,
                nama_wasit: wasit.nama_wasit.clone(),
                posisi: wasit.posisi.clone(),
                conflict_type: "perguruan".to_string(),
                reason: format!(
                    "Bentrok Perguruan: Wasit {} dan Atlet {} ({}) sama-sama dari perguruan {}",
                    wasit.nama_wasit, side, athlete_nama, perg_nama
                ),
                detail_bagan_id: match_id,
                match_kode: match_kode.map(|s| s.to_string()),
                athlete_side: side.to_string(),
                athlete_name: athlete_nama,
                conflicting_value: perg_nama,
            });
        }
    }

    // 2. Daerah / Kontingen Conflict Check:
    // If referee's kab_kota matches athlete's utusan / kontingen name or kab_kota
    if let Some(wasit_daerah) = wasit.kab_kota.as_deref() {
        if is_location_conflict(Some(wasit_daerah), athlete.utusan_nama.as_deref()) {
            let utusan_nama = athlete.utusan_nama.as_deref().unwrap_or(wasit_daerah);
            return Some(WasitConflict {
                wasit_id: wasit.wasit_id,
                nama_wasit: wasit.nama_wasit.clone(),
                posisi: wasit.posisi.clone(),
                conflict_type: "kontingen_daerah".to_string(),
                reason: format!(
                    "Bentrok Daerah: Asal wasit ({}) sama dengan kontingen Atlet {} ({}: {})",
                    wasit_daerah, side, athlete_nama, utusan_nama
                ),
                detail_bagan_id: match_id,
                match_kode: match_kode.map(|s| s.to_string()),
                athlete_side: side.to_string(),
                athlete_name: athlete_nama,
                conflicting_value: wasit_daerah.to_string(),
            });
        }
    }

    None
}

/// Evaluates all conflicts for a given Tatami panel against an active match.
pub fn evaluate_panel_conflicts(
    assignments: &[EnrichedWasitTatami],
    atlet1: Option<&EnrichedAtlet>,
    atlet2: Option<&EnrichedAtlet>,
    match_id: Option<i64>,
    match_kode: Option<&str>,
) -> Vec<WasitConflict> {
    let mut conflicts = Vec::new();

    for wasit in assignments {
        if let Some(a1) = atlet1 {
            if let Some(c) = check_wasit_athlete_conflict(wasit, a1, "AKA", match_id, match_kode) {
                conflicts.push(c);
                continue; // Avoid duplicate alerts for same referee on single athlete
            }
        }

        if let Some(a2) = atlet2 {
            if let Some(c) = check_wasit_athlete_conflict(wasit, a2, "AO", match_id, match_kode) {
                conflicts.push(c);
            }
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_wasit(id: i64, name: &str, perg_id: Option<i64>, perg_name: &str, kab_kota: &str, pos: &str) -> EnrichedWasitTatami {
        EnrichedWasitTatami {
            id,
            event_id: 1,
            tatami_id: 1,
            tatami_number: Some(1),
            wasit_id: id,
            nama_wasit: name.to_string(),
            lisensi: Some("Wasit Nasional A".to_string()),
            no_lisensi: Some("LIS-001".to_string()),
            perguruan_id: perg_id,
            nama_perguruan: Some(perg_name.to_string()),
            kab_kota: Some(kab_kota.to_string()),
            posisi: pos.to_string(),
            assigned_at: None,
            foto: None,
        }
    }

    fn dummy_atlet(id: i64, name: &str, perg_id: Option<i64>, perg_name: &str, utusan: &str) -> EnrichedAtlet {
        EnrichedAtlet {
            id,
            event_id: Some(1),
            nama_atlet: Some(name.to_string()),
            nik: None,
            kode_atlet: None,
            additional_code: None,
            is_priority: false,
            perguruan_id: perg_id,
            perguruan_nama: Some(perg_name.to_string()),
            utusan_id: Some(10),
            utusan_nama: Some(utusan.to_string()),
            utusan_logo: None,
            nomor_tanding_id: Some(1),
            nomor_tanding_nama: Some("Kumite Senior Putra -60kg".to_string()),
            nomor_tanding_tipe: Some("kumite".to_string()),
            is_bob: None,
        }
    }

    #[test]
    fn test_normalize_location() {
        assert_eq!(normalize_location("Kota Surabaya"), "surabaya");
        assert_eq!(normalize_location("Kabupaten Bandung Barat"), "bandung barat");
        assert_eq!(normalize_location("Kontingen DKI Jakarta"), "jakarta");
        assert_eq!(normalize_location("Dojo Shindoka Samarinda"), "shindoka samarinda");
    }

    #[test]
    fn test_is_location_conflict() {
        assert!(is_location_conflict(Some("Kota Surabaya"), Some("Kontingen Surabaya")));
        assert!(is_location_conflict(Some("DKI Jakarta"), Some("Pengprov FORKI Jakarta")));
        assert!(!is_location_conflict(Some("Kota Surabaya"), Some("Kontingen Bandung")));
        assert!(!is_location_conflict(None, Some("Surabaya")));
    }

    #[test]
    fn test_perguruan_conflict_detection() {
        let wasit = dummy_wasit(1, "Sensei Budi", Some(2), "INKANAS", "Kota Bandung", "referee");
        let atlet_aka = dummy_atlet(101, "Atlet Ahmad", Some(2), "INKANAS", "Kontingen Jawa Barat");
        let atlet_ao = dummy_atlet(102, "Atlet John", Some(5), "LEMKARI", "Kontingen Jawa Timur");

        let conflict_aka = check_wasit_athlete_conflict(&wasit, &atlet_aka, "AKA", Some(50), Some("M-01"));
        assert!(conflict_aka.is_some());
        let c = conflict_aka.unwrap();
        assert_eq!(c.conflict_type, "perguruan");
        assert_eq!(c.athlete_side, "AKA");

        let conflict_ao = check_wasit_athlete_conflict(&wasit, &atlet_ao, "AO", Some(50), Some("M-01"));
        assert!(conflict_ao.is_none());
    }

    #[test]
    fn test_daerah_conflict_detection() {
        let wasit = dummy_wasit(2, "Sensei Agus", Some(3), "INKAI", "Kota Surabaya", "judge_1");
        // Athlete has different perguruan (LEMKARI) but same city (Surabaya)
        let atlet_aka = dummy_atlet(201, "Atlet Rian", Some(5), "LEMKARI", "Kontingen Kota Surabaya");

        let conflict = check_wasit_athlete_conflict(&wasit, &atlet_aka, "AKA", Some(51), Some("M-02"));
        assert!(conflict.is_some());
        let c = conflict.unwrap();
        assert_eq!(c.conflict_type, "kontingen_daerah");
    }

    #[test]
    fn test_evaluate_panel_conflicts() {
        let w1 = dummy_wasit(1, "Wasit 1", Some(1), "INKAI", "Jakarta", "referee");
        let w2 = dummy_wasit(2, "Wasit 2", Some(2), "INKANAS", "Surabaya", "judge_1");
        let w3 = dummy_wasit(3, "Wasit 3", Some(3), "LEMKARI", "Medan", "judge_2");
        let panel = vec![w1, w2, w3];

        let atlet_aka = dummy_atlet(10, "Aka Atlet", Some(2), "INKANAS", "Bandung");
        let atlet_ao = dummy_atlet(11, "Ao Atlet", Some(4), "KKI", "Medan");

        let conflicts = evaluate_panel_conflicts(&panel, Some(&atlet_aka), Some(&atlet_ao), Some(1), Some("M-1"));
        // w2 conflicts with aka (INKANAS)
        // w3 conflicts with ao (Medan)
        // w1 has no conflict
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].wasit_id, 2);
        assert_eq!(conflicts[1].wasit_id, 3);
    }
}
