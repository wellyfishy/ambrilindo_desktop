use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, XlsxError};
use crate::models::{
    EnrichedAtlet, EnrichedDetailBagan, EnrichedWasit, SummaryBaganResult, SummaryJuaraResult,
};

pub struct TournamentExcelData {
    pub event_name: String,
    pub summary_juara: SummaryJuaraResult,
    pub summary_bagan: SummaryBaganResult,
    pub athletes: Vec<EnrichedAtlet>,
    pub matches: Vec<EnrichedDetailBagan>,
    pub wasits: Vec<EnrichedWasit>,
}

pub fn generate_tournament_workbook(data: &TournamentExcelData) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();

    // Reusable Formats
    let fmt_title = Format::new()
        .set_bold()
        .set_font_size(16)
        .set_font_name("Segoe UI")
        .set_font_color(Color::RGB(0x1F4E78));

    let fmt_subtitle = Format::new()
        .set_italic()
        .set_font_size(11)
        .set_font_name("Segoe UI")
        .set_font_color(Color::RGB(0x595959));

    let fmt_section_header = Format::new()
        .set_bold()
        .set_font_size(13)
        .set_font_name("Segoe UI")
        .set_font_color(Color::RGB(0x0C2340));

    let fmt_header = Format::new()
        .set_bold()
        .set_font_name("Segoe UI")
        .set_font_size(10)
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1F4E78))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0x8EA9DB));

    let fmt_header_gold = Format::new()
        .set_bold()
        .set_font_name("Segoe UI")
        .set_font_size(10)
        .set_font_color(Color::RGB(0x5C4400))
        .set_background_color(Color::RGB(0xFFE599))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0xD4AF37));

    let fmt_header_silver = Format::new()
        .set_bold()
        .set_font_name("Segoe UI")
        .set_font_size(10)
        .set_font_color(Color::RGB(0x404040))
        .set_background_color(Color::RGB(0xE6E6E6))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0xBFBFBF));

    let fmt_header_bronze = Format::new()
        .set_bold()
        .set_font_name("Segoe UI")
        .set_font_size(10)
        .set_font_color(Color::RGB(0x5A2C10))
        .set_background_color(Color::RGB(0xFBCBAD))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0xCD7F32));

    let fmt_data = Format::new()
        .set_font_name("Segoe UI")
        .set_font_size(10)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0xD9D9D9));

    let fmt_data_center = Format::new()
        .set_font_name("Segoe UI")
        .set_font_size(10)
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0xD9D9D9));

    let fmt_data_bold_center = Format::new()
        .set_bold()
        .set_font_name("Segoe UI")
        .set_font_size(10)
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0xD9D9D9));

    // ==========================================
    // SHEET 1: KLASEMEN MEDALI (JUARA UMUM)
    // ==========================================
    let sheet1 = workbook.add_worksheet();
    sheet1.set_name("Klasemen Medali")?;

    sheet1.write_string_with_format(0, 0, "KLASEMEN PEROLEHAN MEDALI JUARA UMUM", &fmt_title)?;
    sheet1.write_string_with_format(1, 0, &format!("Event: {} • Exported by Ambrilindo Tournament Manager", data.event_name), &fmt_subtitle)?;

    // Table 1: Kontingen Standings
    sheet1.write_string_with_format(3, 0, "A. JUARA UMUM KONTINGEN / UTUSAN", &fmt_section_header)?;

    let kontingen_headers = ["Peringkat", "Nama Kontingen / Utusan", "Emas (Gold)", "Perak (Silver)", "Perunggu (Bronze)", "Total Medali"];
    for (col, text) in kontingen_headers.iter().enumerate() {
        let fmt = match col {
            2 => &fmt_header_gold,
            3 => &fmt_header_silver,
            4 => &fmt_header_bronze,
            _ => &fmt_header,
        };
        sheet1.write_string_with_format(4, col as u16, *text, fmt)?;
    }

    let mut row_idx = 5;
    for item in &data.summary_juara.utusan_standings {
        sheet1.write_number_with_format(row_idx, 0, item.rank as f64, &fmt_data_center)?;
        sheet1.write_string_with_format(row_idx, 1, &item.nama, &fmt_data)?;
        sheet1.write_number_with_format(row_idx, 2, item.gold as f64, &fmt_data_bold_center)?;
        sheet1.write_number_with_format(row_idx, 3, item.silver as f64, &fmt_data_bold_center)?;
        sheet1.write_number_with_format(row_idx, 4, item.bronze as f64, &fmt_data_bold_center)?;
        sheet1.write_number_with_format(row_idx, 5, item.total as f64, &fmt_data_bold_center)?;
        row_idx += 1;
    }

    if data.summary_juara.utusan_standings.is_empty() {
        sheet1.write_string_with_format(row_idx, 1, "Belum ada perolehan medali kontingen.", &fmt_data)?;
        row_idx += 1;
    }

    // Table 2: Perguruan Standings
    row_idx += 2;
    sheet1.write_string_with_format(row_idx, 0, "B. JUARA UMUM PERGURUAN", &fmt_section_header)?;
    row_idx += 1;

    let perguruan_headers = ["Peringkat", "Nama Perguruan", "Emas (Gold)", "Perak (Silver)", "Perunggu (Bronze)", "Total Medali"];
    for (col, text) in perguruan_headers.iter().enumerate() {
        let fmt = match col {
            2 => &fmt_header_gold,
            3 => &fmt_header_silver,
            4 => &fmt_header_bronze,
            _ => &fmt_header,
        };
        sheet1.write_string_with_format(row_idx, col as u16, *text, fmt)?;
    }
    row_idx += 1;

    for item in &data.summary_juara.perguruan_standings {
        sheet1.write_number_with_format(row_idx, 0, item.rank as f64, &fmt_data_center)?;
        sheet1.write_string_with_format(row_idx, 1, &item.nama, &fmt_data)?;
        sheet1.write_number_with_format(row_idx, 2, item.gold as f64, &fmt_data_bold_center)?;
        sheet1.write_number_with_format(row_idx, 3, item.silver as f64, &fmt_data_bold_center)?;
        sheet1.write_number_with_format(row_idx, 4, item.bronze as f64, &fmt_data_bold_center)?;
        sheet1.write_number_with_format(row_idx, 5, item.total as f64, &fmt_data_bold_center)?;
        row_idx += 1;
    }

    if data.summary_juara.perguruan_standings.is_empty() {
        sheet1.write_string_with_format(row_idx, 1, "Belum ada perolehan medali perguruan.", &fmt_data)?;
    }

    sheet1.set_column_width(0, 12)?;
    sheet1.set_column_width(1, 38)?;
    sheet1.set_column_width(2, 14)?;
    sheet1.set_column_width(3, 14)?;
    sheet1.set_column_width(4, 16)?;
    sheet1.set_column_width(5, 16)?;

    // ==========================================
    // SHEET 2: DAFTAR JUARA SELURUH BAGAN
    // ==========================================
    let sheet2 = workbook.add_worksheet();
    sheet2.set_name("Juara Bagan")?;

    sheet2.write_string_with_format(0, 0, "DAFTAR JUARA SELURUH BAGAN PERTANDINGAN", &fmt_title)?;
    sheet2.write_string_with_format(1, 0, &format!("Event: {} • Total Kategori: {}", data.event_name, data.summary_bagan.bagans.len()), &fmt_subtitle)?;

    let bagan_headers = [
        "No",
        "Kategori Pertandingan",
        "Juara 1 (Emas)",
        "Kontingen Juara 1",
        "Juara 2 (Perak)",
        "Kontingen Juara 2",
        "Juara 3 Bersama (A)",
        "Kontingen Juara 3A",
        "Juara 3 Bersama (B)",
        "Kontingen Juara 3B",
    ];

    for (col, text) in bagan_headers.iter().enumerate() {
        let fmt = match col {
            2 | 3 => &fmt_header_gold,
            4 | 5 => &fmt_header_silver,
            6 | 7 | 8 | 9 => &fmt_header_bronze,
            _ => &fmt_header,
        };
        sheet2.write_string_with_format(3, col as u16, *text, fmt)?;
    }

    let mut b_row = 4;
    for (i, b) in data.summary_bagan.bagans.iter().enumerate() {
        sheet2.write_number_with_format(b_row, 0, (i + 1) as f64, &fmt_data_center)?;
        sheet2.write_string_with_format(b_row, 1, &b.nama_bagan, &fmt_data)?;

        let w1 = b.winners.iter().find(|w| w.juara == "1");
        let w2 = b.winners.iter().find(|w| w.juara == "2");
        let w3a = b.winners.iter().find(|w| w.juara == "3a" || w.juara == "3");
        let w3b = b.winners.iter().find(|w| w.juara == "3b");

        // Juara 1
        sheet2.write_string_with_format(b_row, 2, w1.map(|w| w.nama_atlet.as_str()).unwrap_or("-"), &fmt_data)?;
        sheet2.write_string_with_format(b_row, 3, w1.map(|w| w.utusan_nama.as_str()).unwrap_or("-"), &fmt_data)?;

        // Juara 2
        sheet2.write_string_with_format(b_row, 4, w2.map(|w| w.nama_atlet.as_str()).unwrap_or("-"), &fmt_data)?;
        sheet2.write_string_with_format(b_row, 5, w2.map(|w| w.utusan_nama.as_str()).unwrap_or("-"), &fmt_data)?;

        // Juara 3A
        sheet2.write_string_with_format(b_row, 6, w3a.map(|w| w.nama_atlet.as_str()).unwrap_or("-"), &fmt_data)?;
        sheet2.write_string_with_format(b_row, 7, w3a.map(|w| w.utusan_nama.as_str()).unwrap_or("-"), &fmt_data)?;

        // Juara 3B
        sheet2.write_string_with_format(b_row, 8, w3b.map(|w| w.nama_atlet.as_str()).unwrap_or("-"), &fmt_data)?;
        sheet2.write_string_with_format(b_row, 9, w3b.map(|w| w.utusan_nama.as_str()).unwrap_or("-"), &fmt_data)?;

        b_row += 1;
    }

    sheet2.set_column_width(0, 8)?;
    sheet2.set_column_width(1, 35)?;
    sheet2.set_column_width(2, 24)?;
    sheet2.set_column_width(3, 24)?;
    sheet2.set_column_width(4, 24)?;
    sheet2.set_column_width(5, 24)?;
    sheet2.set_column_width(6, 24)?;
    sheet2.set_column_width(7, 24)?;
    sheet2.set_column_width(8, 24)?;
    sheet2.set_column_width(9, 24)?;

    // ==========================================
    // SHEET 3: REKAP ATLET & KONTINGEN
    // ==========================================
    let sheet3 = workbook.add_worksheet();
    sheet3.set_name("Rekap Atlet")?;

    sheet3.write_string_with_format(0, 0, "DAFTAR ATLET & KONTINGEN TERDAFTAR", &fmt_title)?;
    sheet3.write_string_with_format(1, 0, &format!("Event: {} • Total Atlet: {}", data.event_name, data.athletes.len()), &fmt_subtitle)?;

    let athlete_headers = [
        "No",
        "Nama Lengkap Atlet",
        "Kode / NIK",
        "Perguruan",
        "Kontingen / Utusan",
        "Kategori Pertandingan",
        "Tipe",
        "Prioritas",
    ];

    for (col, text) in athlete_headers.iter().enumerate() {
        sheet3.write_string_with_format(3, col as u16, *text, &fmt_header)?;
    }

    let mut a_row = 4;
    for (i, a) in data.athletes.iter().enumerate() {
        sheet3.write_number_with_format(a_row, 0, (i + 1) as f64, &fmt_data_center)?;
        sheet3.write_string_with_format(a_row, 1, a.nama_atlet.as_deref().unwrap_or("-"), &fmt_data)?;
        sheet3.write_string_with_format(a_row, 2, a.kode_atlet.as_deref().or(a.nik.as_deref()).unwrap_or("-"), &fmt_data_center)?;
        sheet3.write_string_with_format(a_row, 3, a.perguruan_nama.as_deref().unwrap_or("-"), &fmt_data)?;
        sheet3.write_string_with_format(a_row, 4, a.utusan_nama.as_deref().unwrap_or("-"), &fmt_data)?;
        sheet3.write_string_with_format(a_row, 5, a.nomor_tanding_nama.as_deref().unwrap_or("-"), &fmt_data)?;
        sheet3.write_string_with_format(a_row, 6, a.nomor_tanding_tipe.as_deref().unwrap_or("-"), &fmt_data_center)?;
        sheet3.write_string_with_format(a_row, 7, if a.is_priority { "Prioritas (Seed)" } else { "Biasa" }, &fmt_data_center)?;
        a_row += 1;
    }

    sheet3.set_column_width(0, 8)?;
    sheet3.set_column_width(1, 28)?;
    sheet3.set_column_width(2, 16)?;
    sheet3.set_column_width(3, 22)?;
    sheet3.set_column_width(4, 25)?;
    sheet3.set_column_width(5, 32)?;
    sheet3.set_column_width(6, 12)?;
    sheet3.set_column_width(7, 16)?;

    // ==========================================
    // SHEET 4: JADWAL & HASIL PERTANDINGAN
    // ==========================================
    let sheet4 = workbook.add_worksheet();
    sheet4.set_name("Hasil Pertandingan")?;

    sheet4.write_string_with_format(0, 0, "JADWAL & HASIL SELURUH PARTAI PERTANDINGAN", &fmt_title)?;
    sheet4.write_string_with_format(1, 0, &format!("Event: {} • Total Partai: {}", data.event_name, data.matches.len()), &fmt_subtitle)?;

    let match_headers = [
        "No",
        "Tatami",
        "Kode Partai",
        "Babak",
        "Sudut Aka (Merah)",
        "Kontingen Aka",
        "Skor Aka",
        "Skor Ao",
        "Kontingen Ao",
        "Sudut Ao (Biru)",
        "Pemenang",
        "Status",
    ];

    for (col, text) in match_headers.iter().enumerate() {
        sheet4.write_string_with_format(3, col as u16, *text, &fmt_header)?;
    }

    let mut m_row = 4;
    for (i, m) in data.matches.iter().enumerate() {
        sheet4.write_number_with_format(m_row, 0, (i + 1) as f64, &fmt_data_center)?;
        let tatami_str = m.assigned_tatami_id.map(|t| format!("Tatami {}", t)).unwrap_or_else(|| "-".to_string());
        sheet4.write_string_with_format(m_row, 1, &tatami_str, &fmt_data_center)?;
        sheet4.write_string_with_format(m_row, 2, m.kode.as_deref().unwrap_or("-"), &fmt_data_center)?;
        sheet4.write_string_with_format(m_row, 3, &format!("Babak {}", m.round.unwrap_or(1)), &fmt_data_center)?;

        sheet4.write_string_with_format(m_row, 4, m.atlet1_nama.as_deref().unwrap_or("BYE"), &fmt_data)?;
        sheet4.write_string_with_format(m_row, 5, m.atlet1_kontingen.as_deref().unwrap_or("-"), &fmt_data)?;
        sheet4.write_string_with_format(m_row, 6, m.score1.as_deref().unwrap_or("-"), &fmt_data_bold_center)?;
        sheet4.write_string_with_format(m_row, 7, m.score2.as_deref().unwrap_or("-"), &fmt_data_bold_center)?;
        sheet4.write_string_with_format(m_row, 8, m.atlet2_kontingen.as_deref().unwrap_or("-"), &fmt_data)?;
        sheet4.write_string_with_format(m_row, 9, m.atlet2_nama.as_deref().unwrap_or("BYE"), &fmt_data)?;

        let pemenang_str = match m.pemenang.as_deref() {
            Some("1") | Some("aka") | Some("AKA") => "AKA (Merah)",
            Some("2") | Some("ao") | Some("AO") => "AO (Biru)",
            Some("draw") => "DRAW",
            _ => if m.selesai { "Selesai" } else { "Belum Selesai" },
        };
        sheet4.write_string_with_format(m_row, 10, pemenang_str, &fmt_data_bold_center)?;
        sheet4.write_string_with_format(m_row, 11, if m.selesai { "SELESAI" } else { "PENDING" }, &fmt_data_center)?;

        m_row += 1;
    }

    sheet4.set_column_width(0, 8)?;
    sheet4.set_column_width(1, 14)?;
    sheet4.set_column_width(2, 14)?;
    sheet4.set_column_width(3, 12)?;
    sheet4.set_column_width(4, 25)?;
    sheet4.set_column_width(5, 22)?;
    sheet4.set_column_width(6, 12)?;
    sheet4.set_column_width(7, 12)?;
    sheet4.set_column_width(8, 22)?;
    sheet4.set_column_width(9, 25)?;
    sheet4.set_column_width(10, 16)?;
    sheet4.set_column_width(11, 14)?;

    // ==========================================
    // SHEET 5: DEWAN WASIT & JURI
    // ==========================================
    let sheet5 = workbook.add_worksheet();
    sheet5.set_name("Dewan Wasit")?;

    sheet5.write_string_with_format(0, 0, "DAFTAR DEWAN WASIT & JURI RESMI", &fmt_title)?;
    sheet5.write_string_with_format(1, 0, &format!("Event: {} • Total Pejabat: {}", data.event_name, data.wasits.len()), &fmt_subtitle)?;

    let wasit_headers = [
        "No",
        "Nama Lengkap Wasit / Juri",
        "Tingkat Lisensi",
        "Nomor Lisensi",
        "Perguruan",
        "Asal Daerah / Kota",
        "Penugasan Tatami",
        "Peran / Posisi",
        "Status",
    ];

    for (col, text) in wasit_headers.iter().enumerate() {
        sheet5.write_string_with_format(3, col as u16, *text, &fmt_header)?;
    }

    let mut w_row = 4;
    for (i, w) in data.wasits.iter().enumerate() {
        sheet5.write_number_with_format(w_row, 0, (i + 1) as f64, &fmt_data_center)?;
        sheet5.write_string_with_format(w_row, 1, &w.nama_wasit, &fmt_data)?;
        sheet5.write_string_with_format(w_row, 2, w.lisensi.as_deref().unwrap_or("-"), &fmt_data_center)?;
        sheet5.write_string_with_format(w_row, 3, w.no_lisensi.as_deref().unwrap_or("-"), &fmt_data_center)?;
        sheet5.write_string_with_format(w_row, 4, w.nama_perguruan.as_deref().unwrap_or("-"), &fmt_data)?;
        sheet5.write_string_with_format(w_row, 5, w.kab_kota.as_deref().unwrap_or("-"), &fmt_data)?;

        let tatami_str = w.assigned_tatami_number.map(|num| format!("Tatami {}", num)).unwrap_or_else(|| "Cadangan / Meja".to_string());
        sheet5.write_string_with_format(w_row, 6, &tatami_str, &fmt_data_center)?;
        sheet5.write_string_with_format(w_row, 7, w.assigned_posisi.as_deref().unwrap_or("-"), &fmt_data_center)?;
        sheet5.write_string_with_format(w_row, 8, if w.is_active { "AKTIF" } else { "NON-AKTIF" }, &fmt_data_center)?;

        w_row += 1;
    }

    sheet5.set_column_width(0, 8)?;
    sheet5.set_column_width(1, 30)?;
    sheet5.set_column_width(2, 20)?;
    sheet5.set_column_width(3, 18)?;
    sheet5.set_column_width(4, 22)?;
    sheet5.set_column_width(5, 24)?;
    sheet5.set_column_width(6, 18)?;
    sheet5.set_column_width(7, 20)?;
    sheet5.set_column_width(8, 14)?;

    workbook.save_to_buffer()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{StandingItem, SummaryBaganResult, SummaryJuaraResult};

    #[test]
    fn test_generate_tournament_workbook_valid_bytes() {
        let dummy_juara = SummaryJuaraResult {
            event_name: "Kejuaraan Karate Piala Gubernur".to_string(),
            utusan_standings: vec![
                StandingItem {
                    id: 1,
                    rank: 1,
                    nama: "Kontingen DKI Jakarta".to_string(),
                    logo: None,
                    gold: 5,
                    silver: 3,
                    bronze: 4,
                    total: 12,
                    uncounted_gold: 0,
                    winners: vec![],
                },
                StandingItem {
                    id: 2,
                    rank: 2,
                    nama: "Kontingen Jawa Barat".to_string(),
                    logo: None,
                    gold: 4,
                    silver: 4,
                    bronze: 2,
                    total: 10,
                    uncounted_gold: 0,
                    winners: vec![],
                },
            ],
            perguruan_standings: vec![
                StandingItem {
                    id: 10,
                    rank: 1,
                    nama: "INKANAS".to_string(),
                    logo: None,
                    gold: 6,
                    silver: 2,
                    bronze: 3,
                    total: 11,
                    uncounted_gold: 0,
                    winners: vec![],
                },
            ],
            total_gold: 15,
            total_silver: 9,
            total_bronze: 9,
            total_uncounted: 0,
        };

        let dummy_bagan = SummaryBaganResult {
            event_name: "Kejuaraan Karate Piala Gubernur".to_string(),
            total_bagans: 0,
            completed_bagans: 0,
            total_gold: 0,
            total_silver: 0,
            total_bronze: 0,
            bagans: vec![],
        };

        let data = TournamentExcelData {
            event_name: "Kejuaraan Karate Piala Gubernur".to_string(),
            summary_juara: dummy_juara,
            summary_bagan: dummy_bagan,
            athletes: vec![],
            matches: vec![],
            wasits: vec![],
        };

        let buffer = generate_tournament_workbook(&data).expect("Workbook generation failed");
        assert!(!buffer.is_empty());
        // XLSX files are zip archives starting with magic byte PK\x03\x04
        assert_eq!(&buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    }

    #[test]
    fn test_results_booklet_data_serialization() {
        use crate::models::{
            ResultsBookletBeritaAcara, ResultsBookletData, ResultsBookletEventInfo, ResultsBookletSignatory,
        };

        let event_info = ResultsBookletEventInfo {
            nama_event: "Piala Presiden Karate 2026".to_string(),
            tanggal: "1 - 3 November 2026".to_string(),
            tempat: "GOR Segiri".to_string(),
            nama_organisasi: "Pengprov FORKI Kaltim".to_string(),
            logo: None,
            alamat: Some("Jl. Kesuma Bangsa No. 1".to_string()),
            kontak: Some("08123456789".to_string()),
            kota: "Samarinda".to_string(),
        };

        let berita_acara = ResultsBookletBeritaAcara {
            nomor_surat: "BA-FORKI/1/2026".to_string(),
            hari_tanggal: "1 - 3 November 2026".to_string(),
            total_kontingen: 25,
            total_perguruan: 8,
            total_atlet: 450,
            total_partai: 120,
            partai_selesai: 120,
            catatan_sanksi: "Nihil".to_string(),
            status_pelaksanaan: "SELESAI 100%".to_string(),
            tanggal_pengesahan: "3 November 2026".to_string(),
            kota_pengesahan: "Samarinda".to_string(),
        };

        let signatories = vec![
            ResultsBookletSignatory {
                jabatan: "Technical Delegate".to_string(),
                peran: "Pengawas WKF".to_string(),
                nama: "Sensei Budi".to_string(),
                gelar: Some("Dan VI".to_string()),
                nomor_lisensi: Some("TD-01".to_string()),
                instansi: Some("FORKI".to_string()),
            },
        ];

        let booklet = ResultsBookletData {
            event_info,
            berita_acara,
            signatories,
            standings_kontingen: vec![],
            standings_perguruan: vec![],
            category_winners: vec![],
            referees: vec![],
            total_gold: 10,
            total_silver: 10,
            total_bronze: 20,
            grand_total: 40,
        };

        let json = serde_json::to_string(&booklet).expect("Serialization failed");
        assert!(json.contains("Piala Presiden Karate 2026"));
        assert!(json.contains("BA-FORKI/1/2026"));
        assert!(json.contains("Sensei Budi"));
        assert_eq!(booklet.grand_total, 40);
    }

    #[test]
    fn test_bob_bracket_models_and_seeding() {
        use crate::models::{BobBaganResult, BobSourceCategory, CreateBobBaganInput};
        use crate::bracket::{build_bracket_in_memory, AthleteInput};

        let cat = BobSourceCategory {
            bagan_id: 101,
            nomor_tanding_id: 10,
            nama_bagan: "KUMITE SENIOR -55 KG PUTRA".to_string(),
            kode_bagan: "001".to_string(),
            juara_1_id: Some(501),
            juara_1_nama: Some("Ahmad Rizky".to_string()),
            juara_1_kontingen: Some("KKI SAMARINDA".to_string()),
            juara_1_perguruan: Some("KKI".to_string()),
            is_completed: true,
        };
        assert!(cat.is_completed);
        assert_eq!(cat.juara_1_id, Some(501));

        let input = CreateBobBaganInput {
            event_id: 1,
            nama_bob: "BEST OF THE BEST KUMITE SENIOR PUTRA".to_string(),
            source_bagan_ids: vec![101, 102, 103, 104],
            shuffle_type: Some("kontingen".to_string()),
            format_type: Some("elimination".to_string()),
        };
        assert_eq!(input.source_bagan_ids.len(), 4);

        let athletes = vec![
            AthleteInput { id: 1, name: "Ahmad".to_string(), is_priority: false, group_key: "U_1".to_string() },
            AthleteInput { id: 2, name: "Budi".to_string(), is_priority: false, group_key: "U_2".to_string() },
            AthleteInput { id: 3, name: "Chandra".to_string(), is_priority: false, group_key: "U_3".to_string() },
            AthleteInput { id: 4, name: "Doni".to_string(), is_priority: false, group_key: "U_4".to_string() },
        ];
        let bracket = build_bracket_in_memory(&athletes, 1, false);
        assert!(bracket.matches.contains_key(&1));
        assert!(bracket.matches.contains_key(&2));

        let res = BobBaganResult {
            nomor_tanding_id: 99,
            bagan_id: 201,
            athlete_count: 4,
            nama_bob: input.nama_bob.clone(),
        };
        assert_eq!(res.athlete_count, 4);
    }
}
