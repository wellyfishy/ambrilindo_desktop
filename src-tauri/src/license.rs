//! Desktop license (Ambrilindo Kata / Kumite / Drawing).
//!
//! A license key is activated once online against ambrilindo_web, which returns a token:
//! `base64url(payload_json) + "." + base64url(ed25519_signature)`. The token is bound to this
//! machine's device id and verified offline with the embedded public key, so the app keeps
//! working without internet after activation.
//!
//! This module is self-contained (no `crate::` imports) so it can be tested on its own.

use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Public half of the key in ambrilindo_web's `LICENSE_SIGNING_KEY`
/// (`python manage.py generate_license_keypair`). Changing it invalidates every activation.
pub const PUBLIC_KEY_B64: &str = "C6tuBXuo0S2FtHQvph26XnVRLKMv2MxEJ7l/VMe+L5Q=";

/// The clock may go back this far (DST, NTP corrections) before we treat it as tampering.
const CLOCK_TOLERANCE_SECS: i64 = 24 * 3600;
const LICENSE_FILE: &str = "license.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Kata,
    Kumite,
    Drawing,
}

impl Module {
    pub fn code(self) -> &'static str {
        match self {
            Module::Kata => "kata",
            Module::Kumite => "kumite",
            Module::Drawing => "drawing",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Module::Kata => "Ambrilindo Kata",
            Module::Kumite => "Ambrilindo Kumite",
            Module::Drawing => "Ambrilindo Drawing",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LicensePayload {
    pub v: u32,
    pub lid: i64,
    pub key_hint: String,
    pub buyer: String,
    #[serde(default)]
    pub org: String,
    pub modules: Vec<String>,
    pub max_devices: u32,
    pub device_id: String,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
}

/// What is stored in `<app config dir>/license.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LicenseFile {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub token: Option<String>,
    /// High-water mark of the wall clock, used to detect the clock being turned back.
    #[serde(default)]
    pub last_seen: i64,
    #[serde(default)]
    pub last_checked_at: Option<i64>,
    /// Set when the server said the key was revoked / this device was released.
    #[serde(default)]
    pub server_message: Option<String>,
}

/// Status sent to the UI (`get_license_status`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    /// none | active | expired | invalid | clock_tampered
    pub state: String,
    pub message: Option<String>,
    pub buyer: Option<String>,
    pub org: Option<String>,
    pub modules: Vec<String>,
    pub max_devices: Option<u32>,
    pub expires_at: Option<i64>,
    pub key_hint: Option<String>,
    pub key: Option<String>,
    pub device_id: String,
    pub device_name: String,
    pub last_checked_at: Option<i64>,
    pub server_url: String,
}

// ---------------------------------------------------------------------------
// Pure logic (unit tested)
// ---------------------------------------------------------------------------

pub fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn public_key() -> VerifyingKey {
    let raw = STANDARD.decode(PUBLIC_KEY_B64).expect("PUBLIC_KEY_B64 is valid base64");
    let arr: [u8; 32] = raw.try_into().expect("PUBLIC_KEY_B64 is 32 bytes");
    VerifyingKey::from_bytes(&arr).expect("PUBLIC_KEY_B64 is a valid ed25519 key")
}

/// Checks the signature and decodes the payload. Does not check device or expiry.
pub fn verify_token(token: &str, key: &VerifyingKey) -> Result<LicensePayload, String> {
    let (body_b64, sig_b64) = token.trim().split_once('.').ok_or("Format token lisensi tidak valid.")?;
    let body = URL_SAFE_NO_PAD.decode(body_b64).map_err(|_| "Token lisensi rusak.")?;
    let sig_raw = URL_SAFE_NO_PAD.decode(sig_b64).map_err(|_| "Tanda tangan lisensi rusak.")?;
    let sig = Signature::from_slice(&sig_raw).map_err(|_| "Tanda tangan lisensi rusak.")?;
    key.verify(&body, &sig).map_err(|_| "Tanda tangan lisensi tidak sah.")?;
    serde_json::from_slice::<LicensePayload>(&body).map_err(|_| "Isi lisensi tidak dikenali.".to_string())
}

/// Turns the stored file into a status, given the current time and this machine's device id.
pub fn evaluate(file: &LicenseFile, now: i64, device_id: &str, key: &VerifyingKey) -> LicenseStatus {
    let mut st = LicenseStatus {
        state: "none".into(),
        device_id: device_id.to_string(),
        last_checked_at: file.last_checked_at,
        key: (!file.key.is_empty()).then(|| file.key.clone()),
        ..Default::default()
    };

    let Some(token) = file.token.as_deref().filter(|t| !t.trim().is_empty()) else {
        st.message = file.server_message.clone();
        return st;
    };

    let payload = match verify_token(token, key) {
        Ok(p) => p,
        Err(e) => {
            st.state = "invalid".into();
            st.message = Some(e);
            return st;
        }
    };

    st.buyer = Some(payload.buyer.clone());
    st.org = Some(payload.org.clone());
    st.modules = payload.modules.clone();
    st.max_devices = Some(payload.max_devices);
    st.expires_at = payload.expires_at;
    st.key_hint = Some(payload.key_hint.clone());

    if !payload.device_id.eq_ignore_ascii_case(device_id) {
        st.state = "invalid".into();
        st.message = Some("Lisensi ini diaktivasi untuk komputer lain. Aktivasi ulang di komputer ini.".into());
        return st;
    }
    if file.last_seen > 0 && now < file.last_seen - CLOCK_TOLERANCE_SECS {
        st.state = "clock_tampered".into();
        st.message = Some("Jam komputer mundur dari pemakaian terakhir. Perbaiki tanggal/jam, lalu klik Perbarui (butuh internet).".into());
        return st;
    }
    if payload.expires_at.is_some_and(|exp| now > exp) {
        st.state = "expired".into();
        st.message = Some("Masa berlaku lisensi sudah habis. Hubungi Ambrilindo untuk perpanjangan, lalu klik Perbarui.".into());
        return st;
    }
    st.state = "active".into();
    st
}

/// Whether `status` unlocks `module` at time `now`.
pub fn check(status: &LicenseStatus, module: Module, now: i64) -> Result<(), String> {
    let base = format!("Modul {} terkunci", module.label());
    match status.state.as_str() {
        "active" => {}
        "none" => return Err(format!("{base}: belum ada lisensi. Buka menu Lisensi untuk aktivasi.")),
        _ => {
            let why = status.message.clone().unwrap_or_else(|| "lisensi tidak berlaku.".into());
            return Err(format!("{base}. {why}"));
        }
    }
    if status.expires_at.is_some_and(|exp| now > exp) {
        return Err(format!("{base}. Masa berlaku lisensi sudah habis."));
    }
    if !status.modules.iter().any(|m| m == module.code()) {
        return Err(format!("{base}: modul ini tidak termasuk dalam lisensi Anda."));
    }
    Ok(())
}

/// Which module a bracket needs, from `tipe_tanding` ("1" kata, "2" kumite) or its name.
/// None when it cannot be told (then either Kata or Kumite is enough).
pub fn discipline_of(tipe_tanding: Option<&str>, name: Option<&str>) -> Option<Module> {
    let tt = tipe_tanding.unwrap_or("").trim().to_lowercase();
    if tt == "1" || tt == "kata" {
        return Some(Module::Kata);
    }
    if tt == "2" || tt == "kumite" {
        return Some(Module::Kumite);
    }
    let n = name.unwrap_or("").to_lowercase();
    if n.contains("kata") {
        return Some(Module::Kata);
    }
    if n.contains("kumite") {
        return Some(Module::Kumite);
    }
    None
}

// ---------------------------------------------------------------------------
// Machine identity and file storage
// ---------------------------------------------------------------------------

pub fn device_id() -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let machine = machine_uid::get().unwrap_or_else(|_| device_name());
        #[allow(unused_mut)]
        let mut seed = format!("ambrilindo-desktop:{machine}");
        // Debug builds only: pretend to be another computer (tests the N-device limit).
        #[cfg(debug_assertions)]
        if let Ok(extra) = std::env::var("AMBRILINDO_DEVICE_ID_SALT") {
            seed.push(':');
            seed.push_str(&extra);
        }
        let digest = Sha256::digest(seed.as_bytes());
        digest.iter().map(|b| format!("{b:02x}")).collect()
    })
    .clone()
}

pub fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Komputer".into())
}

pub fn server_url() -> String {
    if let Ok(url) = std::env::var("AMBRILINDO_LICENSE_URL") {
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.is_empty() {
            return url;
        }
    }
    // Keys are created on the production site, so dev builds activate there too.
    // Local testing: set AMBRILINDO_LICENSE_URL=http://127.0.0.1:8001.
    "https://www.ambrilindo.com".into()
}

fn read_file(dir: &Path) -> LicenseFile {
    std::fs::read_to_string(dir.join(LICENSE_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_file(dir: &Path, file: &LicenseFile) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Gagal membuat folder lisensi: {e}"))?;
    let json = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(LICENSE_FILE), json).map_err(|e| format!("Gagal menyimpan lisensi: {e}"))
}

// ---------------------------------------------------------------------------
// Process-wide state
// ---------------------------------------------------------------------------

static DIR: OnceLock<PathBuf> = OnceLock::new();
static STATUS: OnceLock<RwLock<LicenseStatus>> = OnceLock::new();

fn status_cell() -> &'static RwLock<LicenseStatus> {
    STATUS.get_or_init(|| RwLock::new(LicenseStatus { state: "none".into(), ..Default::default() }))
}

fn decorate(mut st: LicenseStatus) -> LicenseStatus {
    st.device_name = device_name();
    st.server_url = server_url();
    st
}

/// Re-reads the file, moves the clock high-water mark forward and refreshes the cached status.
fn reload() -> LicenseStatus {
    let Some(dir) = DIR.get() else {
        return decorate(LicenseStatus { state: "none".into(), device_id: device_id(), ..Default::default() });
    };
    let mut file = read_file(dir);
    let now = now_secs();
    let st = decorate(evaluate(&file, now, &device_id(), &public_key()));
    // Never move the mark backwards, and don't overwrite it while tampering is detected.
    if st.state != "clock_tampered" && now > file.last_seen {
        file.last_seen = now;
        let _ = write_file(dir, &file);
    }
    *status_cell().write().unwrap() = st.clone();
    st
}

/// Call once at startup with the app config dir.
pub fn init(config_dir: PathBuf) -> LicenseStatus {
    let _ = DIR.set(config_dir);
    reload()
}

pub fn current() -> LicenseStatus {
    status_cell().read().unwrap().clone()
}

/// Credentials for the web sync API: the signed token and this device id, only while the license is
/// active. The web checks the token and lets it touch only the events granted to this license.
pub fn sync_credentials() -> Option<(String, String)> {
    if current().state != "active" {
        return None;
    }
    let token = read_file(DIR.get()?).token?;
    Some((token, device_id()))
}

/// Gate used by the drawing and control-panel code paths.
pub fn ensure(module: Module) -> Result<(), String> {
    #[cfg(test)]
    if !test_enforce::enabled() {
        return Ok(());
    }
    check(&current(), module, now_secs())
}

/// Kata or Kumite: either one is enough when the discipline is unknown.
pub fn ensure_discipline(module: Option<Module>) -> Result<(), String> {
    match module {
        Some(m) => ensure(m),
        None => ensure(Module::Kata).or_else(|_| ensure(Module::Kumite)),
    }
}

#[cfg(test)]
pub mod test_enforce {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ENFORCE: AtomicBool = AtomicBool::new(false);
    /// Other test suites run drawing code without a license; only license tests turn this on.
    pub fn enabled() -> bool {
        ENFORCE.load(Ordering::SeqCst)
    }
    pub fn set(on: bool) {
        ENFORCE.store(on, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// Online calls (activation, refresh, release)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ServerReply {
    token: Option<String>,
    error: Option<String>,
    code: Option<String>,
}

async fn post(path: &str, key: &str) -> Result<(u16, ServerReply), String> {
    let url = format!("{}/api/license/{path}/", server_url());
    let body = serde_json::json!({
        "key": key,
        "device_id": device_id(),
        "device_name": device_name(),
        "os": std::env::consts::OS,
        "app_version": env!("CARGO_PKG_VERSION"),
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|_| "Tidak bisa terhubung ke server lisensi. Periksa koneksi internet lalu coba lagi.".to_string())?;
    let status = res.status().as_u16();
    let reply = res
        .json::<ServerReply>()
        .await
        .map_err(|_| format!("Balasan server lisensi tidak dikenali (HTTP {status})."))?;
    Ok((status, reply))
}

fn dir() -> Result<&'static PathBuf, String> {
    DIR.get().ok_or_else(|| "Lisensi belum siap.".to_string())
}

fn store_token(key: &str, token: String) -> Result<LicenseStatus, String> {
    let dir = dir()?;
    // Make sure the server's token is genuine and for this machine before saving it.
    let payload = verify_token(&token, &public_key())?;
    if !payload.device_id.eq_ignore_ascii_case(&device_id()) {
        return Err("Token dari server tidak cocok dengan komputer ini.".into());
    }
    let mut file = read_file(dir);
    file.key = key.to_string();
    file.token = Some(token);
    file.last_checked_at = Some(now_secs());
    file.server_message = None;
    // A successful online check repairs a clock-tampered state.
    file.last_seen = now_secs();
    write_file(dir, &file)?;
    Ok(reload())
}

/// Activate `key` on this computer (uses one device slot).
pub async fn activate(key: &str) -> Result<LicenseStatus, String> {
    let key = key.trim().to_uppercase().replace(' ', "");
    if key.is_empty() {
        return Err("Masukkan key lisensi.".into());
    }
    let (_, reply) = post("activate", &key).await?;
    match reply.token {
        Some(token) => store_token(&key, token),
        None => Err(reply.error.unwrap_or_else(|| "Aktivasi gagal.".into())),
    }
}

/// Re-fetch the token for the saved key (picks up extensions, module changes, revocation).
/// `Ok(None)` when there is no saved key.
pub async fn refresh() -> Result<Option<LicenseStatus>, String> {
    let dir = dir()?;
    let file = read_file(dir);
    if file.key.is_empty() {
        return Ok(None);
    }
    let (_, reply) = post("refresh", &file.key).await?;
    if let Some(token) = reply.token {
        return store_token(&file.key, token).map(Some);
    }
    let message = reply.error.unwrap_or_else(|| "Lisensi tidak berlaku.".into());
    match reply.code.as_deref() {
        // The server no longer honours this activation: drop the token so it stops working offline too.
        Some("revoked") | Some("not_activated") | Some("not_found") => {
            let mut file = read_file(dir);
            file.token = None;
            file.last_checked_at = Some(now_secs());
            file.server_message = Some(message.clone());
            write_file(dir, &file)?;
            reload();
            Err(message)
        }
        _ => Err(message),
    }
}

/// Free this computer's slot on the server and remove the local license.
pub async fn deactivate() -> Result<LicenseStatus, String> {
    let dir = dir()?;
    let file = read_file(dir);
    if !file.key.is_empty() {
        post("deactivate", &file.key).await?;
    }
    let cleared = LicenseFile { last_seen: file.last_seen, ..Default::default() };
    write_file(dir, &cleared)?;
    Ok(reload())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    // Same seed as LicenseApiTests in ambrilindo_web (bytes 0..31).
    fn test_signing_key() -> SigningKey {
        let seed: [u8; 32] = core::array::from_fn(|i| i as u8);
        SigningKey::from_bytes(&seed)
    }

    // Signed by ambrilindo_web's backend.licensing.sign_token with the test seed.
    const PY_FIXTURE: &str = "eyJidXllciI6IkZpeHR1cmUgQnV5ZXIiLCJkZXZpY2VfaWQiOiJkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkIiwiZXhwaXJlc19hdCI6NDEwMjQ0NDgwMCwiaXNzdWVkX2F0IjoxNzAwMDAwMDAwLCJrZXlfaGludCI6IkFCQ0RFIiwibGlkIjo3LCJtYXhfZGV2aWNlcyI6MiwibW9kdWxlcyI6WyJrYXRhIiwiZHJhd2luZyJdLCJvcmciOiJPcmciLCJ2IjoxfQ.jTA8UL8RRz5uKY6D6er9e6ujCL9x8UcNWo74cxZibmILAjBlLyw1-iL0JxWyPwYx6fGuy5jBRHUly22ssK3wCQ";

    fn sign(payload: &LicensePayload) -> String {
        let body = serde_json::to_vec(payload).unwrap();
        let sig = test_signing_key().sign(&body);
        format!("{}.{}", URL_SAFE_NO_PAD.encode(&body), URL_SAFE_NO_PAD.encode(sig.to_bytes()))
    }

    fn payload(device: &str, modules: &[&str], expires_at: Option<i64>) -> LicensePayload {
        LicensePayload {
            v: 1,
            lid: 1,
            key_hint: "XXXXX".into(),
            buyer: "Buyer".into(),
            org: "Org".into(),
            modules: modules.iter().map(|s| s.to_string()).collect(),
            max_devices: 1,
            device_id: device.into(),
            issued_at: 1_000,
            expires_at,
        }
    }

    fn file_with(token: String, last_seen: i64) -> LicenseFile {
        LicenseFile { key: "AMBR-TEST".into(), token: Some(token), last_seen, ..Default::default() }
    }

    const DEV: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

    #[test]
    fn python_fixture_verifies() {
        let key = test_signing_key().verifying_key();
        let p = verify_token(PY_FIXTURE, &key).expect("python token verifies in rust");
        assert_eq!(p.modules, vec!["kata", "drawing"]);
        assert_eq!(p.device_id, DEV);
        assert_eq!(p.expires_at, Some(4_102_444_800));
        let st = evaluate(&file_with(PY_FIXTURE.into(), 0), 1_800_000_000, DEV, &key);
        assert_eq!(st.state, "active");
        assert!(check(&st, Module::Kata, 1_800_000_000).is_ok());
        assert!(check(&st, Module::Kumite, 1_800_000_000).is_err());
    }

    #[test]
    fn embedded_public_key_parses() {
        let _ = public_key();
    }

    #[test]
    fn tampered_payload_rejected() {
        let key = test_signing_key().verifying_key();
        let token = sign(&payload(DEV, &["kata"], None));
        let (body, sig) = token.split_once('.').unwrap();
        let mut raw = URL_SAFE_NO_PAD.decode(body).unwrap();
        let s = String::from_utf8(raw.clone()).unwrap().replace("\"kata\"", "\"kumi\"");
        raw = s.into_bytes();
        let forged = format!("{}.{}", URL_SAFE_NO_PAD.encode(raw), sig);
        assert!(verify_token(&forged, &key).is_err());
        assert_eq!(evaluate(&file_with(forged, 0), 2_000, DEV, &key).state, "invalid");
    }

    #[test]
    fn other_device_rejected() {
        let key = test_signing_key().verifying_key();
        let token = sign(&payload("e".repeat(64).as_str(), &["kata"], None));
        let st = evaluate(&file_with(token, 0), 2_000, DEV, &key);
        assert_eq!(st.state, "invalid");
        assert!(check(&st, Module::Kata, 2_000).is_err());
    }

    #[test]
    fn expiry_locks_immediately() {
        let key = test_signing_key().verifying_key();
        let token = sign(&payload(DEV, &["kata", "kumite", "drawing"], Some(5_000)));
        let active = evaluate(&file_with(token.clone(), 0), 4_999, DEV, &key);
        assert_eq!(active.state, "active");
        // Status computed before expiry still locks once time passes it.
        assert!(check(&active, Module::Drawing, 5_001).is_err());
        assert_eq!(evaluate(&file_with(token, 0), 5_001, DEV, &key).state, "expired");
    }

    #[test]
    fn clock_rollback_detected() {
        let key = test_signing_key().verifying_key();
        let token = sign(&payload(DEV, &["kata"], None));
        let last_seen = 10_000_000;
        assert_eq!(evaluate(&file_with(token.clone(), last_seen), last_seen - 3_600, DEV, &key).state, "active");
        let st = evaluate(&file_with(token, last_seen), last_seen - 3 * 24 * 3600, DEV, &key);
        assert_eq!(st.state, "clock_tampered");
        assert!(check(&st, Module::Kata, last_seen).is_err());
    }

    #[test]
    fn no_license_locks_everything() {
        let key = test_signing_key().verifying_key();
        let st = evaluate(&LicenseFile::default(), 1, DEV, &key);
        assert_eq!(st.state, "none");
        for m in [Module::Kata, Module::Kumite, Module::Drawing] {
            assert!(check(&st, m, 1).is_err());
        }
    }

    #[test]
    fn discipline_detection() {
        assert_eq!(discipline_of(Some("1"), Some("Kumite -60")), Some(Module::Kata));
        assert_eq!(discipline_of(Some("2"), None), Some(Module::Kumite));
        assert_eq!(discipline_of(None, Some("KATA PERORANGAN PUTRA")), Some(Module::Kata));
        assert_eq!(discipline_of(Some(""), Some("Kumite +84")), Some(Module::Kumite));
        assert_eq!(discipline_of(None, Some("Festival")), None);
    }
}
