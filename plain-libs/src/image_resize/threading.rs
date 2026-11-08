use std::num::NonZeroU32;

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;

use crate::image_resize::pixels::InnerPixel;
use crate::image_resize::{ImageView, ImageViewMut};

const ONE: NonZeroU32 = NonZeroU32::MIN;

#[inline]
fn non_zero_or_one(num: u32) -> NonZeroU32 {
    NonZeroU32::new(num).unwrap_or(ONE)
}

#[inline]
pub(crate) fn split_h_two_images_for_threading<'a, P: InnerPixel>(
    src_view: &'a impl ImageView<Pixel = P>,
    dst_view: &'a mut impl ImageViewMut<Pixel = P>,
    src_offset: u32,
) -> Option<
    impl Iterator<
        Item = (
            impl ImageView<Pixel = P> + 'a,
            impl ImageViewMut<Pixel = P> + 'a,
        ),
    >,
> {
    debug_assert!(src_view.height() - src_offset >= dst_view.height());
    let dst_width = dst_view.width();
    let dst_height = dst_view.height();

    let num_threads = *PARALLELISM;
    let max_width = dst_width.max(src_view.width());
    let num_parts =
        calculate_max_number_of_horizontal_parts(max_width, dst_height).min(num_threads);
    if num_parts > ONE {
        let dst_height = NonZeroU32::new(dst_height)?;
        if let Some(src_parts) = src_view.split_by_height(src_offset, dst_height, num_parts) {
            if let Some(dst_parts) = dst_view.split_by_height_mut(0, dst_height, num_parts) {
                let src_iter = src_parts.into_iter();
                let dst_iter = dst_parts.into_iter();
                return Some(src_iter.zip(dst_iter));
            }
        }
    }
    None
}

#[inline]
pub(crate) fn split_h_one_image_for_threading<P: InnerPixel>(
    image_view: &mut impl ImageViewMut<Pixel = P>,
) -> Option<impl Iterator<Item = impl ImageViewMut<Pixel = P> + '_>> {
    let width = image_view.width();
    let height = image_view.height();

    let num_threads = *PARALLELISM;
    let num_parts = calculate_max_number_of_horizontal_parts(width, height).min(num_threads);
    if num_parts > ONE {
        let height = NonZeroU32::new(height)?;
        let img_parts = image_view.split_by_height_mut(0, height, num_parts);
        return img_parts.map(|parts| parts.into_iter());
    }
    None
}

#[inline]
pub(crate) fn split_v_two_images_for_threading<'a, P: InnerPixel>(
    src_view: &'a impl ImageView<Pixel = P>,
    dst_view: &'a mut impl ImageViewMut<Pixel = P>,
    src_offset: u32,
) -> Option<
    impl Iterator<
        Item = (
            impl ImageView<Pixel = P> + 'a,
            impl ImageViewMut<Pixel = P> + 'a,
        ),
    >,
> {
    debug_assert!(src_view.width() - src_offset >= dst_view.width());

    let dst_width = dst_view.width();
    let dst_height = dst_view.height();

    let num_threads = *PARALLELISM;
    let max_height = dst_height.max(src_view.height());
    let num_parts = calculate_max_number_of_vertical_parts(dst_width, max_height).min(num_threads);
    if num_parts > ONE {
        let dst_width = NonZeroU32::new(dst_width).unwrap();
        if let Some(src_parts) = src_view.split_by_width(src_offset, dst_width, num_parts) {
            if let Some(dst_parts) = dst_view.split_by_width_mut(0, dst_width, num_parts) {
                let src_iter = src_parts.into_iter();
                let dst_iter = dst_parts.into_iter();
                return Some(src_iter.zip(dst_iter));
            }
        }
    }
    None
}

const PIXELS_PER_THREAD: u64 = 64 * 1024;

static PARALLELISM: LazyLock<NonZeroU32> = LazyLock::new(|| {
    non_zero_or_one(
        thread::available_parallelism()
            .ok()
            .and_then(|threads| u32::try_from(threads.get()).ok())
            .unwrap_or(1),
    )
});
static ACTIVE_WORKERS: AtomicU32 = AtomicU32::new(0);

struct Worker;

impl Worker {
    fn reserve() -> Option<Self> {
        ACTIVE_WORKERS
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |active| {
                (active < PARALLELISM.get() - 1).then_some(active + 1)
            })
            .ok()
            .map(|_| Self)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::Relaxed);
    }
}

pub(crate) fn for_each_part<T: Send>(mut parts: impl Iterator<Item = T>, apply: impl Fn(T) + Sync) {
    let Some(first) = parts.next() else {
        return;
    };
    thread::scope(|scope| {
        let apply = &apply;
        for part in parts {
            if let Some(worker) = Worker::reserve() {
                scope.spawn(move || {
                    let _worker = worker;
                    apply(part);
                });
            } else {
                apply(part);
            }
        }
        apply(first);
    });
}

fn calculate_max_number_of_horizontal_parts(width: u32, height: u32) -> NonZeroU32 {
    let area = width as u64 * height as u64;
    let num_parts = (area / PIXELS_PER_THREAD).min(height as _) as u32;
    non_zero_or_one(num_parts)
}

fn calculate_max_number_of_vertical_parts(width: u32, height: u32) -> NonZeroU32 {
    let area = width as u64 * height as u64;
    let num_parts = (area / PIXELS_PER_THREAD).min(width as _) as u32;
    non_zero_or_one(num_parts)
}
