#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub(super) struct Chromaticity {
    pub(super) x: f32,
    pub(super) y: f32,
}
impl Chromaticity {
    #[inline]
    pub(super) const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    #[inline]
    const fn to_scaled_xyz(&self) -> Xyz {
        let z = 1.0 - self.x - self.y;
        Xyz::new(self.x, self.y, z)
    }
    #[inline]
    const fn to_xyyb(&self) -> XyY {
        XyY {
            x: self.x as f64,
            y: self.y as f64,
            yb: 1.,
        }
    }
    pub(super) const D65: Chromaticity = Chromaticity {
        x: 0.31272,
        y: 0.32903,
    };
}
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub(super) struct ColorPrimaries {
    pub(super) red: Chromaticity,
    pub(super) green: Chromaticity,
    pub(super) blue: Chromaticity,
}
impl ColorPrimaries {
    pub(super) const DCI_P3: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.680, y: 0.320 },
        green: Chromaticity { x: 0.265, y: 0.690 },
        blue: Chromaticity { x: 0.150, y: 0.060 },
    };
    pub(super) const DISPLAY_P3: ColorPrimaries = Self::DCI_P3;
    pub(super) const BT_709: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.64, y: 0.33 },
        green: Chromaticity { x: 0.30, y: 0.60 },
        blue: Chromaticity { x: 0.15, y: 0.06 },
    };
    pub(super) const BT_470M: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.67, y: 0.33 },
        green: Chromaticity { x: 0.21, y: 0.71 },
        blue: Chromaticity { x: 0.14, y: 0.08 },
    };
    pub(super) const BT_470BG: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.64, y: 0.33 },
        green: Chromaticity { x: 0.29, y: 0.60 },
        blue: Chromaticity { x: 0.15, y: 0.06 },
    };
    pub(super) const BT_601: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.630, y: 0.340 },
        green: Chromaticity { x: 0.310, y: 0.595 },
        blue: Chromaticity { x: 0.155, y: 0.070 },
    };
    pub(super) const SMPTE_240: ColorPrimaries = Self::BT_601;
    pub(super) const GENERIC_FILM: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.681, y: 0.319 },
        green: Chromaticity { x: 0.243, y: 0.692 },
        blue: Chromaticity { x: 0.145, y: 0.049 },
    };
    pub(super) const BT_2020: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.708, y: 0.292 },
        green: Chromaticity { x: 0.170, y: 0.797 },
        blue: Chromaticity { x: 0.131, y: 0.046 },
    };
    pub(super) const XYZ: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 1.0, y: 0.0 },
        green: Chromaticity { x: 0.0, y: 1.0 },
        blue: Chromaticity { x: 0.0, y: 0.0 },
    };
    pub(super) const EBU_3213: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.630, y: 0.340 },
        green: Chromaticity { x: 0.295, y: 0.605 },
        blue: Chromaticity { x: 0.155, y: 0.077 },
    };
}
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(super) struct Matrix3f {
    pub(super) v: [[f32; 3]; 3],
}
#[derive(Clone, Copy)]
struct Vector3f {
    v: [f32; 3],
}
impl Matrix3f {
    const IDENTITY: Matrix3f = Matrix3f {
        v: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    #[inline]
    const fn determinant(&self) -> Option<f32> {
        let v = self.v;
        let a0 = v[0][0] * v[1][1] * v[2][2];
        let a1 = v[0][1] * v[1][2] * v[2][0];
        let a2 = v[0][2] * v[1][0] * v[2][1];
        let s0 = v[0][2] * v[1][1] * v[2][0];
        let s1 = v[0][1] * v[1][0] * v[2][2];
        let s2 = v[0][0] * v[1][2] * v[2][1];
        let j = a0 + a1 + a2 - s0 - s1 - s2;
        if j == 0. {
            return None;
        }
        Some(j)
    }
    #[inline]
    const fn inverse(&self) -> Self {
        let v = self.v;
        let det = self.determinant();
        match det {
            None => Matrix3f::IDENTITY,
            Some(determinant) => {
                let det = 1. / determinant;
                let a = v[0][0];
                let b = v[0][1];
                let c = v[0][2];
                let d = v[1][0];
                let e = v[1][1];
                let f = v[1][2];
                let g = v[2][0];
                let h = v[2][1];
                let i = v[2][2];
                Matrix3f {
                    v: [
                        [
                            (e * i - f * h) * det,
                            (c * h - b * i) * det,
                            (b * f - c * e) * det,
                        ],
                        [
                            (f * g - d * i) * det,
                            (a * i - c * g) * det,
                            (c * d - a * f) * det,
                        ],
                        [
                            (d * h - e * g) * det,
                            (b * g - a * h) * det,
                            (a * e - b * d) * det,
                        ],
                    ],
                }
            }
        }
    }
    #[inline]
    const fn mul_row_vector<const R: usize>(&self, rhs: Vector3f) -> Self {
        if R == 0 {
            Self {
                v: [
                    (Vector3f { v: self.v[0] }.const_mul_vector(rhs)).v,
                    self.v[1],
                    self.v[2],
                ],
            }
        } else if R == 1 {
            Self {
                v: [
                    self.v[0],
                    (Vector3f { v: self.v[1] }.const_mul_vector(rhs)).v,
                    self.v[2],
                ],
            }
        } else if R == 2 {
            Self {
                v: [
                    self.v[0],
                    self.v[1],
                    (Vector3f { v: self.v[2] }.const_mul_vector(rhs)).v,
                ],
            }
        } else {
            unimplemented!()
        }
    }
    #[inline]
    const fn mul_vector(&self, other: Vector3f) -> Vector3f {
        let x = self.v[0][1] * other.v[1] + self.v[0][2] * other.v[2] + self.v[0][0] * other.v[0];
        let y = self.v[1][0] * other.v[0] + self.v[1][1] * other.v[1] + self.v[1][2] * other.v[2];
        let z = self.v[2][0] * other.v[0] + self.v[2][1] * other.v[1] + self.v[2][2] * other.v[2];
        Vector3f { v: [x, y, z] }
    }
}
const fn rgb_to_xyz(xyz_matrix: Matrix3f, wp: Xyz) -> Matrix3f {
    let xyz_inverse = xyz_matrix.inverse();
    let s = xyz_inverse.mul_vector(wp.to_vector());
    let mut v = xyz_matrix.mul_row_vector::<0>(s);
    v = v.mul_row_vector::<1>(s);
    v.mul_row_vector::<2>(s)
}
struct Xyz {
    x: f32,
    y: f32,
    z: f32,
}
impl Xyz {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    const fn to_vector(self) -> Vector3f {
        Vector3f {
            v: [self.x, self.y, self.z],
        }
    }
}
#[derive(Clone, Debug, Copy, Default)]
struct XyY {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) yb: f64,
}
impl XyY {
    #[inline]
    const fn to_xyz(self) -> Xyz {
        let reciprocal = if self.y != 0. {
            1. / self.y * self.yb
        } else {
            0.
        };
        Xyz {
            x: (self.x * reciprocal) as f32,
            y: self.yb as f32,
            z: ((1. - self.x - self.y) * reciprocal) as f32,
        }
    }
}

impl Vector3f {
    const fn const_mul_vector(self, rhs: Self) -> Self {
        Self {
            v: [
                self.v[0] * rhs.v[0],
                self.v[1] * rhs.v[1],
                self.v[2] * rhs.v[2],
            ],
        }
    }
}

impl ColorPrimaries {
    /// Returns RGB -> XYZ conversion matrix
    ///
    /// # Arguments
    ///
    /// * `white_point`: [Chromaticity] or [crate::XyY] or any item conforming [XyYRepresentable]
    ///
    /// returns: [Matrix3f]
    pub(super) fn transform_to_xyz(self, white_point: Chromaticity) -> Matrix3f {
        let red_xyz = self.red.to_scaled_xyz();
        let green_xyz = self.green.to_scaled_xyz();
        let blue_xyz = self.blue.to_scaled_xyz();

        let xyz_matrix = Matrix3f {
            v: [
                [red_xyz.x, green_xyz.x, blue_xyz.x],
                [red_xyz.y, green_xyz.y, blue_xyz.y],
                [red_xyz.z, green_xyz.z, blue_xyz.z],
            ],
        };
        rgb_to_xyz(xyz_matrix, white_point.to_xyyb().to_xyz())
    }
}
