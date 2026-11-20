//! PDF: every page of the document is a page of the book.
//!
//! Most comic PDFs are scans: each page is one JPEG and nothing else. Such a
//! page is handed out as that JPEG, untouched. Any other page is drawn and
//! handed out as a new JPEG.

use std::fs::File;
use std::io::Read;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::content::ops::TypedInstruction as Op;
use hayro::hayro_syntax::object::{Array, Dict, Name, Stream};
use hayro::hayro_syntax::page::{Page, Rotation};
use hayro::hayro_syntax::{Filter, LoadPdfError, Pdf as Document};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use image::ExtendedColorType;
use image::codecs::jpeg::JpegEncoder;

use super::{ArchiveError, MAX_PAGE_BYTES, PageEntry};

/// Longer side, in pixels, of a page that has to be drawn.
const DRAWN_LONG_SIDE: f32 = 2200.0;

/// Quality of the JPEG a drawn page becomes.
const DRAWN_QUALITY: u8 = 90;

/// How far a picture's shape on the page may be from its own, as a share,
/// and still count as shown in its own proportions.
const SAME_SHAPE: f32 = 0.02;

pub(super) struct Pdf {
    document: Document,
}

impl Pdf {
    /// Reads the whole file: a PDF cannot be understood in part.
    pub(super) fn open(mut file: File) -> Result<Self, ArchiveError> {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let document = Document::new(bytes).map_err(|error| match error {
            LoadPdfError::Decryption(_) => ArchiveError::Protected,
            LoadPdfError::Invalid => ArchiveError::Pdf,
        })?;
        Ok(Self { document })
    }

    /// The pages, named by their number. A PDF keeps no checksum of a page,
    /// so one is computed here, over the JPEG a page is or over what the
    /// page draws.
    pub(super) fn files(&self) -> Vec<PageEntry> {
        (1..)
            .zip(self.document.pages().iter())
            .map(|(number, page)| {
                let mut checksum = crc32fast::Hasher::new();
                let mut byte_size = 0;
                let mut add = |bytes: &[u8]| {
                    checksum.update(bytes);
                    byte_size += bytes.len() as u64;
                };
                match jpeg_of(page) {
                    Some(jpeg) => add(&jpeg.raw_data()),
                    None => drawn_with(page, &mut add),
                }
                PageEntry {
                    name: format!("{number:05}.jpg"),
                    byte_size,
                    crc32: checksum.finalize(),
                }
            })
            .collect()
    }

    pub(super) fn read(&self, entry: &str) -> Result<Vec<u8>, ArchiveError> {
        let page = self.page(entry).ok_or(ArchiveError::NoSuchEntry)?;
        match jpeg_of(page) {
            Some(jpeg) => {
                let bytes = jpeg.raw_data();
                if bytes.len() as u64 > MAX_PAGE_BYTES {
                    return Err(ArchiveError::PageTooLarge);
                }
                Ok(bytes.into_owned())
            }
            None => draw(page),
        }
    }

    pub(super) fn dimensions(&self, entry: &str) -> Option<(u32, u32)> {
        let page = self.page(entry)?;
        match jpeg_of(page) {
            Some(jpeg) => Some((jpeg.dict().get("Width")?, jpeg.dict().get("Height")?)),
            None => {
                let (_, width, height) = drawn_size(page)?;
                Some((width.into(), height.into()))
            }
        }
    }

    /// The page an entry name stands for.
    fn page(&self, entry: &str) -> Option<&Page<'_>> {
        let number: usize = entry.strip_suffix(".jpg")?.parse().ok()?;
        self.document.pages().get(number.checked_sub(1)?)
    }
}

/// The JPEG a page is, when the page is nothing but that one picture, drawn
/// upright and in its own proportions. Only then is the picture alone the
/// same as the page.
fn jpeg_of<'a>(page: &Page<'a>) -> Option<Stream<'a>> {
    if !matches!(page.rotation(), Rotation::None) {
        return None;
    }
    // How the page's units map to what is drawn: `[a, b, c, d]` of the
    // current transformation. Where on the page it lands does not matter.
    let mut transform = [1.0_f32, 0.0, 0.0, 1.0];
    let mut saved = Vec::new();
    let mut drawn = None;
    let mut operations = page.typed_operations();
    while let Some(operation) = operations.next() {
        match operation {
            Op::SaveState(_) => saved.push(transform),
            Op::RestoreState(_) => transform = saved.pop()?,
            Op::Transform(by) => {
                let [a, b, c, d] = transform;
                let (m_a, m_b) = (by.0.as_f32(), by.1.as_f32());
                let (m_c, m_d) = (by.2.as_f32(), by.3.as_f32());
                transform = [
                    m_a * a + m_b * c,
                    m_a * b + m_b * d,
                    m_c * a + m_d * c,
                    m_c * b + m_d * d,
                ];
            }
            Op::XObject(named) => {
                if drawn.is_some() {
                    return None;
                }
                drawn = Some((page.resources().get_x_object(named.0)?, transform));
            }
            // Marks for tools that read the document's structure; they draw
            // nothing.
            Op::BeginMarkedContent(_)
            | Op::BeginMarkedContentWithProperties(_)
            | Op::EndMarkedContent(_)
            | Op::MarkedContentPoint(_)
            | Op::MarkedContentPointWithProperties(_) => {}
            // Anything else draws, clips or changes how things are drawn.
            _ => return None,
        }
    }
    let (image, [a, b, c, d]) = drawn?;

    let dict = image.dict();
    let is_image = dict
        .get::<Name<'_>>("Subtype")
        .is_some_and(|kind| &*kind == b"Image");
    let filters = image.filters();
    let is_jpeg = filters.len() == 1 && matches!(filters[0], Filter::DctDecode);
    // A mask or a remapping of its values makes the picture on the page
    // differ from the JPEG.
    let altered = ["SMask", "Mask", "Decode"]
        .iter()
        .any(|key| dict.contains_key(key))
        || dict.get::<bool>("ImageMask") == Some(true);
    if !is_image || !is_jpeg || altered || !plain_colours(dict) {
        return None;
    }

    let (width, height): (u32, u32) = (dict.get("Width")?, dict.get("Height")?);
    let upright = a > 0.0 && d > 0.0 && b.abs() < a * 1e-3 && c.abs() < d * 1e-3;
    if !upright || width == 0 || height == 0 {
        return None;
    }
    let shape_on_page = a / d;
    let own_shape = width as f32 / height as f32;
    ((shape_on_page / own_shape - 1.0).abs() <= SAME_SHAPE).then_some(image)
}

/// Whether a picture's colours are the grey or red-green-blue a browser
/// shows a JPEG in, so that the JPEG alone looks as the page does.
fn plain_colours(dict: &Dict<'_>) -> bool {
    if let Some(name) = dict.get::<Name<'_>>("ColorSpace") {
        return matches!(
            &*name,
            b"DeviceRGB" | b"DeviceGray" | b"CalRGB" | b"CalGray"
        );
    }
    // A colour profile, given as `[/ICCBased profile]`: plain when the
    // profile is for one or three components.
    let Some(space) = dict.get::<Array<'_>>("ColorSpace") else {
        return false;
    };
    let mut parts = space.flex_iter();
    let is_profile = parts
        .next::<Name<'_>>()
        .is_some_and(|kind| &*kind == b"ICCBased");
    let components = parts
        .next::<Stream<'_>>()
        .and_then(|profile| profile.dict().get::<u8>("N"));
    is_profile && matches!(components, Some(1 | 3))
}

/// Hands `add` everything a page draws with: its instructions, and the
/// pictures and forms they name. That stands in for the page's content when
/// the page is not one JPEG.
fn drawn_with(page: &Page<'_>, add: &mut impl FnMut(&[u8])) {
    add(page.page_stream().unwrap_or_default());
    let mut operations = page.typed_operations();
    while let Some(operation) = operations.next() {
        if let Op::XObject(named) = operation
            && let Some(object) = page.resources().get_x_object(named.0)
        {
            add(&object.raw_data());
        }
    }
}

/// The scale a page is drawn at, and the width and height that gives.
/// `None` for a page with no size.
fn drawn_size(page: &Page<'_>) -> Option<(f32, u16, u16)> {
    let (width, height) = page.render_dimensions();
    let longer = width.max(height);
    if !(width > 0.0 && height > 0.0 && longer.is_finite()) {
        return None;
    }
    let scale = DRAWN_LONG_SIDE / longer;
    // As the renderer rounds them.
    let (width, height) = ((width * scale) as u16, (height * scale) as u16);
    (width > 0 && height > 0).then_some((scale, width, height))
}

/// Draws a page on white and encodes it as a JPEG.
fn draw(page: &Page<'_>) -> Result<Vec<u8>, ArchiveError> {
    let (scale, _, _) = drawn_size(page).ok_or(ArchiveError::NotDrawn)?;
    let pixmap = hayro::render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: WHITE,
        },
    );
    // Drawn on white, every pixel is opaque: dropping the fourth value of
    // each leaves its colour.
    let rgb: Vec<u8> = pixmap
        .data_as_u8_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[red, green, blue, _]| [red, green, blue])
        .collect();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, DRAWN_QUALITY)
        .encode(
            &rgb,
            pixmap.width().into(),
            pixmap.height().into(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|_| ArchiveError::NotDrawn)?;
    Ok(jpeg)
}
