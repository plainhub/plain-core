//! [`decode`]: container to pixels.
//!
//! Resolving the primary item, splitting NAL units, decoding each coded
//! picture through [`crate::hevc::decode_still`], composing a grid, converting
//! colour, applying transforms — in that order, and with the declared size
//! checked against the caller's ceiling before any of it allocates.

use alloc::vec::Vec;

use crate::color::{self, Source};
use crate::context::Context;
use crate::error::{Error, Result};
use crate::grid::{self, Grid, Mosaic};
use crate::hevc::{self, Frame};
use crate::image::{DecodeOptions, Image, check_pixels};
use crate::parallel;
use crate::props::ItemProps;
use crate::props::colr::Nclx;
use crate::transform;

/// Decode the primary image of a HEIF file.
///
/// # Errors
///
/// Container-level failures are reported before anything is decoded, so a
/// malformed file costs nothing. A file whose bitstream uses a coding tool
/// this decoder does not implement comes back as [`Error::Unsupported`] naming
/// that tool.
pub fn decode(bytes: &[u8], options: &DecodeOptions) -> Result<Image> {
    decode_with(bytes, options, &decode_coded)
}

pub type CodedDecoder<'a> = dyn Fn(&Context<'_>, u32, &ItemProps<'_>) -> Result<Frame> + Sync + 'a;

pub fn decode_with(
    bytes: &[u8],
    options: &DecodeOptions,
    decoder: &CodedDecoder<'_>,
) -> Result<Image> {
    let ctx = Context::open(bytes)?;
    decode_context(&ctx, options, decoder)
}

/// Decode an opened container through the supplied picture decoder.
pub fn decode_context(
    ctx: &Context<'_>,
    options: &DecodeOptions,
    decoder: &CodedDecoder<'_>,
) -> Result<Image> {
    let threads = options.threads;
    let id = ctx.meta.primary;
    let p = ctx.props(id)?;
    if options.strict && ctx.meta.props.unknown_essential(id).is_some() {
        return Err(Error::Unsupported(
            "the primary item carries an essential property this build does not implement",
        ));
    }
    let limit = options.pixel_limit();
    let (cw, ch) = ctx.coded_size(id, &p)?;
    // Refuse the declared size before decoding anything, so that a small file
    // claiming an enormous image costs nothing.
    check_pixels(cw, ch, limit)?;
    let (tw, th) = transform::transformed_size(cw, ch, &p.transforms)?;
    check_pixels(tw, th, limit)?;

    let decoded = decode_item(&ctx, id, &p, threads, decoder)?;
    let alpha = alpha_frame(&ctx, id, options, limit, threads, decoder)?;
    let nclx = p
        .nclx
        .or_else(|| match &decoded {
            Decoded::Picture(f) => f.color,
            Decoded::Tiles(_, tiles) => tiles.first().and_then(|f| f.color),
        })
        .unwrap_or_default();
    let (layout, alpha) = (options.layout, alpha.as_ref());
    let image = match &decoded {
        Decoded::Picture(frame) => {
            color::convert_source(&Source::Frame(frame), alpha, nclx, layout, limit, threads)?
        }
        // The tiles are read in place: a canvas would be allocated, zeroed
        // and filled only for the colour pass to read it straight back.
        Decoded::Tiles(g, tiles) => {
            let mosaic = Mosaic::new(g, tiles)?;
            color::convert_source(
                &Source::Mosaic(&mosaic),
                alpha,
                nclx,
                layout,
                limit,
                threads,
            )?
        }
    };
    if options.apply_transforms {
        transform::apply_all(image, &p.transforms)
    } else {
        Ok(image)
    }
}

/// What decoding an item produced.
enum Decoded {
    /// One coded picture, validated.
    Picture(Frame),
    /// A grid's tiles in `dimg` order, each validated, not yet composed.
    Tiles(Grid, Vec<Frame>),
}

impl Decoded {
    /// One frame, composing a grid's tiles if there are any.
    fn into_frame(self, limit: u64) -> Result<Frame> {
        match self {
            Decoded::Picture(frame) => Ok(frame),
            Decoded::Tiles(g, tiles) => grid::compose(&g, &tiles, limit),
        }
    }
}

/// Decode one item, following a `grid` derivation when there is one.
///
/// `p` is the item's own properties, gathered once by the caller; a single
/// coded picture is decoded straight from them rather than reading its
/// `iprp` entry a second time. A grid's tiles are independent coded pictures:
/// nothing in one tile's bitstream refers to another, so they are decoded
/// together when the caller allows it. The tiles come back in `dimg` order,
/// which is what [`Mosaic`] and [`grid::compose`] place them by, so the result
/// does not depend on the order they happened to finish in.
fn decode_item(
    ctx: &Context<'_>,
    id: u32,
    p: &ItemProps<'_>,
    threads: Option<usize>,
    decoder: &CodedDecoder<'_>,
) -> Result<Decoded> {
    match ctx.grid(id)? {
        Some((g, tiles)) => {
            let frames = parallel::try_map(&tiles, threads, |t| decoder(ctx, *t, &ctx.props(*t)?))?;
            Ok(Decoded::Tiles(g, frames))
        }
        None => decoder(ctx, id, p).map(Decoded::Picture),
    }
}

/// Decode a single coded picture item through the codec seam.
pub fn decode_coded(ctx: &Context<'_>, id: u32, p: &ItemProps<'_>) -> Result<Frame> {
    let hvcc = p.hvcc.as_ref().ok_or(Error::MissingBox("hvcC"))?;
    let data = ctx.item_data(id)?;
    let nals = hvcc.split_nals(&data)?;
    let params = hvcc.parameter_sets();
    if params.is_empty() {
        return Err(Error::Malformed("hvcC carries no parameter sets"));
    }
    let frame = hevc::decode_still_limited(&params, &nals, crate::DEFAULT_MAX_PIXELS)?;
    frame.validate()?;
    if !p
        .transforms
        .iter()
        .any(|t| matches!(t, crate::props::Transform::Crop(_)))
        && p.ispe
            .is_some_and(|size| (size.width, size.height) != (frame.width, frame.height))
    {
        return Err(Error::Malformed("HEVC dimensions disagree with ispe"));
    }
    Ok(frame)
}

/// Decode the alpha auxiliary item, when the caller asked for alpha and the
/// requested layout has somewhere to put it.
fn alpha_frame(
    ctx: &Context<'_>,
    id: u32,
    options: &DecodeOptions,
    limit: u64,
    threads: Option<usize>,
    decoder: &CodedDecoder<'_>,
) -> Result<Option<Frame>> {
    if !options.decode_alpha || !options.layout.has_alpha() {
        return Ok(None);
    }
    let Some(aux) = ctx.alpha_item(id)? else {
        return Ok(None);
    };
    let aux_props = ctx.props(aux)?;
    // The alpha kernel reads one plane, so a tiled alpha item is composed;
    // alpha grids are rare enough that the canvas is not worth avoiding.
    decode_item(ctx, aux, &aux_props, threads, decoder)?
        .into_frame(limit)
        .map(Some)
}

/// The colour description that will be used for an item, including the
/// documented fallback for files that carry no `colr`.
pub fn effective_nclx(p: &ItemProps<'_>) -> Nclx {
    p.nclx.unwrap_or_default()
}
