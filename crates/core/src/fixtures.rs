//! Small generated comics for tests. Nothing here reads a real library.

use std::io::{Cursor, Write};
use std::path::Path;

use image::{ImageFormat, ImageReader, Rgb, RgbImage};
use rars::{ArchiveVersion, Builder};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// A PNG page of one flat colour. Different shades give different pages.
pub fn page(width: u32, height: u32, shade: u8) -> Vec<u8> {
    encoded(width, height, shade, ImageFormat::Png)
}

/// A JPEG page of one flat colour.
pub fn jpeg_page(width: u32, height: u32, shade: u8) -> Vec<u8> {
    encoded(width, height, shade, ImageFormat::Jpeg)
}

fn encoded(width: u32, height: u32, shade: u8, format: ImageFormat) -> Vec<u8> {
    let image = RgbImage::from_pixel(width, height, Rgb([shade, 255 - shade, shade / 2]));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, format)
        .expect("encoding to memory");
    bytes.into_inner()
}

/// Writes a CBZ with the given entries, creating its folder if needed.
pub fn write_cbz(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, SimpleFileOptions::default())
            .expect("starting an entry");
        zip.write_all(bytes).expect("writing an entry");
    }
    let packed = zip.finish().expect("finishing the archive");
    write_file(path, packed.get_ref());
}

/// A `ComicInfo.xml` with the given fields, to put in an archive under that
/// name.
pub fn comic_info(fields: &[(&str, &str)]) -> Vec<u8> {
    let fields: String = fields
        .iter()
        .map(|(name, value)| format!("<{name}>{value}</{name}>"))
        .collect();
    format!("<?xml version=\"1.0\"?><ComicInfo>{fields}</ComicInfo>").into_bytes()
}

/// Writes a CBR with the given entries, packed the way most CBR files are.
pub fn write_cbr(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let mut rar = Builder::new(ArchiveVersion::Rar29);
    for (name, bytes) in entries {
        rar.add_bytes(name.as_bytes().to_vec(), bytes.clone(), None, None)
            .expect("adding an entry");
    }
    write_file(path, &rar.to_bytes().expect("packing the archive"));
}

/// Writes a file, creating its folder if needed.
fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).expect("creating the folder");
    }
    std::fs::write(path, bytes).expect("writing the file");
}

/// One page of a generated PDF.
pub enum PdfPage {
    /// A JPEG that fills the page, as in a scan.
    Scan(Vec<u8>),
    /// A page of this width and height with a rectangle drawn on it, and no
    /// picture.
    Drawing(u32, u32),
}

/// Writes a PDF with the given pages.
pub fn write_pdf(path: &Path, pages: &[PdfPage]) {
    // Object 1 is the catalog and 2 the page list. Each page then takes
    // three: the page, what it draws, and its picture (empty for a drawing).
    let mut objects: Vec<Vec<u8>> = vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(), Vec::new()];
    let mut kids = String::new();
    for page in pages {
        let first = objects.len() + 1;
        kids.push_str(&format!("{first} 0 R "));
        let (width, height, draws, picture) = match page {
            PdfPage::Scan(jpeg) => {
                let (width, height) =
                    ImageReader::with_format(Cursor::new(jpeg), ImageFormat::Jpeg)
                        .into_dimensions()
                        .expect("reading the size of the picture");
                let mut picture = format!(
                    "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} \
                     /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode \
                     /Length {} >>\nstream\n",
                    jpeg.len()
                )
                .into_bytes();
                picture.extend_from_slice(jpeg);
                picture.extend_from_slice(b"\nendstream");
                (
                    width,
                    height,
                    format!("q {width} 0 0 {height} 0 0 cm /Im0 Do Q"),
                    picture,
                )
            }
            PdfPage::Drawing(width, height) => (
                *width,
                *height,
                format!("0.2 0.4 0.8 rg 0 0 {} {} re f", width / 2, height / 2),
                b"<< >>".to_vec(),
            ),
        };
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] \
                 /Contents {} 0 R /Resources << /XObject << /Im0 {} 0 R >> >> >>",
                first + 1,
                first + 2
            )
            .into_bytes(),
        );
        objects.push(
            format!("<< /Length {} >>\nstream\n{draws}\nendstream", draws.len()).into_bytes(),
        );
        objects.push(picture);
    }
    objects[1] = format!("<< /Type /Pages /Kids [ {kids}] /Count {} >>", pages.len()).into_bytes();

    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (number, object) in (1..).zip(&objects) {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let table_at = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{table_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    write_file(path, &pdf);
}

/// Writes a CBZ of `pages` pages. `seed` makes its pages, and so its
/// identity, different from other books.
pub fn write_book(path: &Path, pages: u8, seed: u8) {
    let entries: Vec<(String, Vec<u8>)> = (0..pages)
        .map(|number| {
            (
                format!("page {:03}.png", number + 1),
                page(40, 60, seed.wrapping_mul(31).wrapping_add(number)),
            )
        })
        .collect();
    let entries: Vec<(&str, Vec<u8>)> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.clone()))
        .collect();
    write_cbz(path, &entries);
}

/// Writes a CBZ of one page whose `ComicInfo.xml` holds `fields`. `seed`
/// makes its page, and so its identity, different from other books.
pub fn write_tagged_book(path: &Path, seed: u8, fields: &[(&str, &str)]) {
    write_cbz(
        path,
        &[
            ("01.png", page(40, 60, seed)),
            ("ComicInfo.xml", comic_info(fields)),
        ],
    );
}
