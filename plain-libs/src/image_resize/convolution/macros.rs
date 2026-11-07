macro_rules! constify_imm8 {
    ($imm8:expr, $expand:ident) => {
        match ($imm8) & 0b0011_1111 {
            0 => {}
            1 => $expand!(1),
            2 => $expand!(2),
            3 => $expand!(3),
            4 => $expand!(4),
            5 => $expand!(5),
            6 => $expand!(6),
            7 => $expand!(7),
            8 => $expand!(8),
            9 => $expand!(9),
            10 => $expand!(10),
            12 => $expand!(12),
            13 => $expand!(13),
            14 => $expand!(14),
            15 => $expand!(15),
            16 => $expand!(16),
            17 => $expand!(17),
            18 => $expand!(18),
            19 => $expand!(19),
            20 => $expand!(20),
            21 => $expand!(21),
            22 => $expand!(22),
            23 => $expand!(23),
            24 => $expand!(24),
            25 => $expand!(25),
            26 => $expand!(26),
            27 => $expand!(27),
            28 => $expand!(28),
            29 => $expand!(29),
            30 => $expand!(30),
            31 => $expand!(31),
            _ => unreachable!(),
        }
    };
}

#[cfg(target_arch = "aarch64")]
macro_rules! constify_64_imm8 {
    ($imm8:expr, $expand:ident) => {
        match ($imm8) & 0b0111_1111 {
            0 => {}
            1 => $expand!(1),
            2 => $expand!(2),
            3 => $expand!(3),
            4 => $expand!(4),
            5 => $expand!(5),
            6 => $expand!(6),
            7 => $expand!(7),
            8 => $expand!(8),
            9 => $expand!(9),
            10 => $expand!(10),
            12 => $expand!(12),
            13 => $expand!(13),
            14 => $expand!(14),
            15 => $expand!(15),
            16 => $expand!(16),
            17 => $expand!(17),
            18 => $expand!(18),
            19 => $expand!(19),
            20 => $expand!(20),
            21 => $expand!(21),
            22 => $expand!(22),
            23 => $expand!(23),
            24 => $expand!(24),
            25 => $expand!(25),
            26 => $expand!(26),
            27 => $expand!(27),
            28 => $expand!(28),
            29 => $expand!(29),
            30 => $expand!(30),
            31 => $expand!(31),
            32 => $expand!(32),
            33 => $expand!(33),
            34 => $expand!(34),
            35 => $expand!(35),
            36 => $expand!(36),
            37 => $expand!(37),
            38 => $expand!(38),
            39 => $expand!(39),
            40 => $expand!(40),
            41 => $expand!(41),
            42 => $expand!(42),
            43 => $expand!(43),
            44 => $expand!(44),
            45 => $expand!(45),
            46 => $expand!(46),
            47 => $expand!(47),
            48 => $expand!(48),
            49 => $expand!(49),
            50 => $expand!(50),
            51 => $expand!(51),
            52 => $expand!(52),
            53 => $expand!(53),
            54 => $expand!(54),
            55 => $expand!(55),
            56 => $expand!(56),
            57 => $expand!(57),
            58 => $expand!(58),
            59 => $expand!(59),
            60 => $expand!(60),
            61 => $expand!(61),
            62 => $expand!(62),
            63 => $expand!(63),
            _ => unreachable!(),
        }
    };
}

macro_rules! impl_u8_convolution {
    ($pixel:ty, $components:literal) => {
        impl Convolution for $pixel {
            fn horiz_convolution(
                src_view: &impl ImageView<Pixel = Self>,
                dst_view: &mut impl ImageViewMut<Pixel = Self>,
                offset: u32,
                coeffs: Coefficients,
                cpu_extensions: CpuExtensions,
            ) {
                debug_assert!(src_view.height() - offset >= dst_view.height());

                let normalizer = Normalizer16::new(coeffs);
                let normalizer_ref = &normalizer;

                try_process_in_threads_h! {
                    horiz_convolution(
                        src_view,
                        dst_view,
                        offset,
                        normalizer_ref,
                        cpu_extensions,
                    );
                }
            }

            fn vert_convolution(
                src_view: &impl ImageView<Pixel = Self>,
                dst_view: &mut impl ImageViewMut<Pixel = Self>,
                offset: u32,
                coeffs: Coefficients,
                cpu_extensions: CpuExtensions,
            ) {
                debug_assert!(src_view.width() - offset >= dst_view.width());

                let normalizer = Normalizer16::new(coeffs);
                let normalizer_ref = &normalizer;

                try_process_in_threads_v! {
                    vert_convolution_u8(
                        src_view,
                        dst_view,
                        offset,
                        normalizer_ref,
                        cpu_extensions,
                    );
                }
            }
        }

        fn horiz_convolution(
            src_view: &impl ImageView<Pixel = $pixel>,
            dst_view: &mut impl ImageViewMut<Pixel = $pixel>,
            offset: u32,
            normalizer: &Normalizer16,
            cpu_extensions: CpuExtensions,
        ) {
            match cpu_extensions {
                #[cfg(target_arch = "x86_64")]
                CpuExtensions::Avx2 => {
                    avx2::horiz_convolution(src_view, dst_view, offset, normalizer)
                }
                #[cfg(target_arch = "x86_64")]
                CpuExtensions::Sse4_1 => {
                    sse4::horiz_convolution(src_view, dst_view, offset, normalizer)
                }
                #[cfg(target_arch = "aarch64")]
                CpuExtensions::Neon => {
                    neon::horiz_convolution(src_view, dst_view, offset, normalizer)
                }
                #[cfg(target_arch = "wasm32")]
                CpuExtensions::Simd128 => {
                    wasm32::horiz_convolution(src_view, dst_view, offset, normalizer)
                }
                _ => native_horiz_convolution(src_view, dst_view, offset, normalizer),
            }
        }

        #[forbid(unsafe_code)]
        fn native_horiz_convolution(
            src_view: &impl ImageView<Pixel = $pixel>,
            dst_view: &mut impl ImageViewMut<Pixel = $pixel>,
            offset: u32,
            normalizer: &Normalizer16,
        ) {
            let initial = 1i32 << (normalizer.precision() - 1);
            let rows = src_view.iter_rows(offset).zip(dst_view.iter_rows_mut(0));
            for (src_row, dst_row) in rows {
                for (chunk, dst_pixel) in normalizer.chunks().iter().zip(dst_row) {
                    let mut sums = [initial; $components];
                    let src_pixels = &src_row[chunk.start as usize..];
                    for (&coefficient, src_pixel) in chunk.values().iter().zip(src_pixels) {
                        for (sum, component) in sums.iter_mut().zip(src_pixel.0) {
                            *sum += component as i32 * coefficient as i32;
                        }
                    }
                    dst_pixel.0 = sums.map(|sum| normalizer.clip(sum));
                }
            }
        }
    };
}
