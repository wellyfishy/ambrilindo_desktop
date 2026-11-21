//! HTML -> PDF with headless Microsoft Edge (results booklet, bracket roster).
//!
//! The page sends its images once each as `assets` (name -> data URL) and refers to them as
//! `assets/<name>` in the HTML. Before, every logo of every bracket was inlined as its own data
//! URL: one booklet became a single HTML of hundreds of MB, which froze the app while it was
//! built and sent, and the save dialog only opened afterwards.

use base64::Engine;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Edge gets this long to print; a hung Edge is killed instead of blocking forever.
const EDGE_TIMEOUT_SECS: u64 = 300;

fn edge_exe() -> Result<&'static str, String> {
    [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ]
    .into_iter()
    .find(|p| Path::new(p).exists())
    .ok_or_else(|| "Microsoft Edge tidak ditemukan di sistem ini untuk rendering PDF.".to_string())
}

/// Only plain file names inside the job folder (no paths, no "..").
fn safe_asset_name(name: &str) -> Option<&str> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        && !name.contains("..");
    ok.then_some(name)
}

fn decode_data_url(data_url: &str) -> Option<Vec<u8>> {
    let (_, b64) = data_url.split_once(";base64,")?;
    base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()
}

/// Renders `html` (+ its assets) to a PDF at `target`. Uses its own temp folder and its own Edge
/// profile, so an Edge window the user has open never makes the headless print wait.
pub async fn render_html_to_pdf(html: &str, assets: &HashMap<String, String>, target: &Path) -> Result<(), String> {
    let edge = edge_exe()?;
    let job = std::env::temp_dir().join(format!(
        "ambrilindo_pdf_{}_{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
    ));
    let asset_dir = job.join("assets");
    std::fs::create_dir_all(&asset_dir).map_err(|e| format!("Gagal membuat folder sementara: {}", e))?;
    let result = async {
        for (name, data) in assets {
            let (Some(name), Some(bytes)) = (safe_asset_name(name), decode_data_url(data)) else { continue };
            std::fs::write(asset_dir.join(name), bytes).map_err(|e| format!("Gagal menulis gambar sementara: {}", e))?;
        }
        let html_path = job.join("index.html");
        std::fs::write(&html_path, html).map_err(|e| format!("Gagal menulis temporary HTML: {}", e))?;

        // Edge prints into the job folder (a plain temp path it can always write); the app then
        // copies the PDF to the chosen place, so a refused or locked target gives a real error
        // (access denied, file open in a viewer, protected folder) instead of a silent no-PDF.
        let job_pdf = job.join("out.pdf");
        let file_url = format!("file:///{}", html_path.to_string_lossy().replace('\\', "/"));
        let mut cmd = tokio::process::Command::new(edge);
        cmd.arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-extensions")
            .arg("--enable-logging=stderr")
            .arg(format!("--user-data-dir={}", job.join("profile").to_string_lossy()))
            .arg("--no-pdf-header-footer")
            .arg(format!("--print-to-pdf={}", job_pdf.to_string_lossy()))
            .arg(&file_url)
            // Windows starts apps opened from Explorer with __COMPAT_LAYER=DetectorsAppHealth,
            // and children inherit it. Edge under any compat layer quits in 0.1 s without a
            // PDF and without a word ("Edge selesai tanpa membuat file PDF"), so it never gets it.
            .env_remove("__COMPAT_LAYER")
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            // no console window flashing up
            cmd.creation_flags(0x0800_0000);
        }
        let output = tokio::time::timeout(std::time::Duration::from_secs(EDGE_TIMEOUT_SECS), cmd.output())
            .await
            .map_err(|_| format!("Edge tidak selesai membuat PDF dalam {} detik.", EDGE_TIMEOUT_SECS))?
            .map_err(|e| format!("Gagal menjalankan Edge PDF printer: {}", e))?;
        let log = format!(
            "exit: {:?}
--- stderr ---
{}
--- stdout ---
{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        let _ = std::fs::write(job.join("edge_log.txt"), &log);
        if !job_pdf.exists() {
            let tail: String = String::from_utf8_lossy(&output.stderr).lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
            return Err(format!(
                "Edge selesai tanpa membuat file PDF (kode keluar {:?}). {} File kerja disimpan untuk diperiksa: {}",
                output.status.code(),
                tail,
                job.to_string_lossy()
            ));
        }
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::copy(&job_pdf, target).map_err(|e| {
            format!(
                "PDF sudah dibuat, tetapi gagal disalin ke {}: {}. Tutup file PDF itu bila sedang terbuka, atau pilih folder lain.",
                target.to_string_lossy(),
                e
            )
        })?;
        Ok(())
    }
    .await;
    // Keep the job folder when Edge made nothing, so the run can be checked; else clean up.
    let keep = matches!(&result, Err(e) if e.contains("File kerja disimpan"));
    if !keep {
        let _ = std::fs::remove_dir_all(&job);
    }
    result
}

/// A temp PDF path for the LAN API (the bytes go back over HTTP).
pub fn temp_pdf_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "ambrilindo_api_booklet_{}.pdf",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_stay_inside_the_job_folder() {
        assert_eq!(safe_asset_name("img_3.png"), Some("img_3.png"));
        assert_eq!(safe_asset_name("../x.png"), None);
        assert_eq!(safe_asset_name("a/b.png"), None);
        assert_eq!(safe_asset_name("C:\\x.png"), None);
        assert_eq!(safe_asset_name(""), None);
    }

    /// Real Edge print with one image asset (Windows with Edge only; run with --ignored).
    #[tokio::test]
    #[ignore]
    async fn edge_prints_html_with_assets() {
        // 1x1 red PNG
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
        let mut assets = HashMap::new();
        assets.insert("img_0.png".to_string(), png.to_string());
        let html = "<html><body><h1>Uji PDF</h1><img src=\"assets/img_0.png\" width=\"50\" height=\"50\"></body></html>";
        let target = std::env::temp_dir().join(format!("ambrilindo_pdf_test_{}.pdf", std::process::id()));
        let started = std::time::Instant::now();
        render_html_to_pdf(html, &assets, &target).await.expect("pdf");
        let bytes = std::fs::read(&target).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "a PDF file");
        eprintln!("PDF {} bytes in {:?}", bytes.len(), started.elapsed());
        let _ = std::fs::remove_file(&target);
    }

    /// A booklet-sized job: many pages, many repeated logo references, target path with spaces.
    #[tokio::test]
    #[ignore]
    async fn edge_prints_a_large_document() {
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
        let mut assets = HashMap::new();
        for i in 0..40 {
            assets.insert(format!("img_{i}.png"), png.to_string());
        }
        let mut html = String::from("<html><head><style>@page{size:A4 landscape;margin:6mm} .p{page-break-after:always}</style></head><body>");
        for page in 0..250 {
            html.push_str(&format!("<div class=\"p\"><h2>Bagan {page}</h2>"));
            for k in 0..32 {
                html.push_str(&format!("<div>Atlet {k} <img src=\"assets/img_{}.png\" width=16 height=16></div>", k % 40));
            }
            html.push_str("</div>");
        }
        html.push_str("</body></html>");
        // As when the app is opened from Explorer: Windows sets this and children inherit it.
        unsafe { std::env::set_var("__COMPAT_LAYER", "DetectorsAppHealth") };
        let dir = std::env::temp_dir().join("ambrilindo pdf uji spasi");
        let target = dir.join("Buku Hasil PORPROV.pdf");
        let started = std::time::Instant::now();
        let res = render_html_to_pdf(&html, &assets, &target).await;
        eprintln!("LARGE result={:?} in {:?} size={:?}", res, started.elapsed(), std::fs::metadata(&target).map(|m| m.len()));
        let _ = std::fs::remove_dir_all(&dir);
        res.unwrap();
    }

    #[test]
    fn data_urls_decode() {
        assert_eq!(decode_data_url("data:image/png;base64,aGk="), Some(b"hi".to_vec()));
        assert_eq!(decode_data_url("http://x/y.png"), None);
    }
}
