#[macro_use]
extern crate itertools;
use core::ops::Deref;
use image::DynamicImage;
use image::{GenericImageView, ImageBuffer, Pixel, Rgba, RgbaImage};
use rand::distributions::Standard;
use rand::prelude::*;
use std::convert::TryInto;
use std::num::Wrapping;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
/// An image rustifier - mainly a parallel implementation of corruster
struct CLI {
    /// The image to be "corrupted" by the program
    #[structopt(parse(from_os_str))]
    image: PathBuf,
    /// The path where the files should be downloaded
    #[structopt(parse(from_os_str))]
    output: PathBuf,
    /// Strength of the blur
    #[structopt(default_value = "7")]
    magnitude: i64,
    /// Average height of a distorted block
    #[structopt(default_value = "15")]
    block_height: u32,
    /// Average offset of a distorted block
    #[structopt(default_value = "30")]
    block_offset: u32,
    /// Strength of the distorted block stride
    #[structopt(default_value = "0.1")]
    stride_magnitude: f64,
    /// Per-channel scanline lag strength
    #[structopt(default_value = "0.05")]
    lag: f64,
    /// Initial red scanline lag
    #[structopt(default_value = "12")]
    lr: u32,
    /// Initial green scanline lag
    #[structopt(default_value = "17")]
    lg: u32,
    /// Initial blue scanline lag
    #[structopt(default_value = "18")]
    lb: u32,
    /// Standard deviation of the red-blue channel offset (non-destructive)
    #[structopt(default_value = "10")]
    std_offset: u32,
    /// Additional brightness control
    #[structopt(default_value = "3")]
    brighteness_addition: u8,
    /// Mean chromatic abberation offset
    #[structopt(default_value = "10")]
    mean_abberation: u32,
    /// Standard deviation of the chromatic abberation offset (lower values induce longer trails)
    #[structopt(default_value = "1")]
    std_abberation: u32,
}

/// Dummy structure holding the four dimensiosn of the input image
#[derive(Clone, Copy)]
struct Bounds {
    x_min: u32,
    y_min: u32,
    x_max: u32,
    y_max: u32,
}

/// Structure holding the image buffer for encapsulation purposes
struct Corrupter {
    bounds: Bounds,
    img: DynamicImage,
    buffer: RgbaImage,
}

/// The primary way we are going to shift color channel
fn modified_pixel(coord: u32, offset_coord: u32, bounds_coord: u32) -> u32 {
    ((Wrapping(coord.clone()) + Wrapping(offset_coord.clone())).0 as i64
        % bounds_coord.clone() as i64)
        .try_into()
        .unwrap()
}

/// A map between the magnitude and the pixel spatial shift
fn offset<T, S>(rng: &mut ThreadRng, magnitude: S) -> T
where
    rand::distributions::Standard: rand::distributions::Distribution<T>,
    T: std::convert::From<S>,
    Wrapping<T>: std::ops::Mul<Output = Wrapping<T>>,
    S: std::convert::Into<T>,
{
    let random = rng.sample::<T, _>(Standard);
    (Wrapping(random) * Wrapping(magnitude.into())).0
}

fn brighten_pixels(pixel: u8, brighteness_addition: u8) -> u8 {
    (Wrapping(pixel) - Wrapping(pixel) * Wrapping(brighteness_addition) / Wrapping(255)
        + Wrapping(brighteness_addition))
    .0
}

impl Corrupter {
    fn new(src_img: &PathBuf) -> Self {
        let img = image::open(src_img).expect("No compatible image found");
        let (x_min, x_max, y_min, y_max) = (0, img.width(), 0, img.height());
        let bounds = Bounds {
            x_min,
            x_max,
            y_min,
            y_max,
        };
        Self {
            bounds,
            img,
            buffer: ImageBuffer::new(bounds.x_max, bounds.y_max),
        }
    }

    /// First stage of the corrupter
    /// Goes through the image and offset some pixel spatially by blocks
    fn dissolve_block(&mut self, rng: &mut ThreadRng, cfg: &CLI) -> &mut Self {
        let mut line_offset = 0;
        let mut stride = 0.;
        let mut yset = 0;

        for (x, y) in iproduct!(
            self.bounds.x_min..self.bounds.x_max,
            self.bounds.y_min..self.bounds.y_max
        ) {
            if rng.gen_ratio(cfg.block_height, self.bounds.x_max) {
                line_offset = offset::<i64, i64>(rng, cfg.block_offset.into());
                stride = cfg.stride_magnitude;
                yset = y;
            }
            let stride_offset: i64 = (stride as u32 * (Wrapping(y) - Wrapping(yset)).0) as i64;
            let offset_x = (Wrapping(offset::<i64, i64>(rng, cfg.magnitude))
                + Wrapping(line_offset)
                + Wrapping(stride_offset))
            .0;
            let offset_y = offset::<i64, i64>(rng, cfg.magnitude);
            self.buffer.put_pixel(
                x,
                y,
                self.img.get_pixel(
                    modified_pixel(
                        x,
                        offset_x.try_into().unwrap_or(std::u32::MAX),
                        self.bounds.x_max,
                    ),
                    modified_pixel(
                        y,
                        offset_y.try_into().unwrap_or(std::u32::MAX),
                        self.bounds.y_max,
                    ),
                ),
            );
        }
        self
    }
    fn random_brightening(&mut self, rng: &mut ThreadRng, cfg: &CLI) -> &mut Self {
        for (x, y) in iproduct!(
            self.bounds.x_min..self.bounds.x_max,
            self.bounds.y_min..self.bounds.y_max
        ) {
            let mut lr = cfg.lr;
            let mut lg = cfg.lg;
            let mut lb = cfg.lb;
            lr += offset::<u32, u32>(rng, cfg.lr.into());
            lg += offset::<u32, u32>(rng, cfg.lg.into());
            lb += offset::<u32, u32>(rng, cfg.lb.into());
            let offset_x = offset::<u32, u32>(rng, cfg.std_offset);
            let [r, _, _, a] = self
                .img
                .get_pixel(
                    modified_pixel((Wrapping(x) - Wrapping(lr)).0, offset_x, self.bounds.x_max),
                    modified_pixel(0, y, self.bounds.y_max),
                )
                .to_rgba()
                .data;

            let (b, g) = (
                self.img
                    .get_pixel(
                        modified_pixel(x, lg, self.bounds.x_max),
                        modified_pixel(0, y, self.bounds.y_max),
                    )
                    .to_rgba()
                    .data[1],
                self.img
                    .get_pixel(
                        modified_pixel(x + lb, offset_x, self.bounds.x_max),
                        modified_pixel(0, y, self.bounds.y_max),
                    )
                    .to_rgba()
                    .data[2],
            );
            self.buffer.put_pixel(
                x,
                y,
                Rgba([
                    brighten_pixels(r, cfg.brighteness_addition),
                    brighten_pixels(g, cfg.brighteness_addition),
                    brighten_pixels(b, cfg.brighteness_addition),
                    brighten_pixels(a, cfg.brighteness_addition),
                ]),
            );
        }
        self
    }
    fn chromatic_abberations(&mut self, rng: &mut ThreadRng, cfg: &CLI) -> &mut Self {
        let offset_x = (Wrapping(cfg.mean_abberation) + Wrapping(offset::<u32, u32>(rng, cfg.std_abberation))).0;
        for (x, y) in iproduct!(
            self.bounds.x_min..self.bounds.x_max,
            self.bounds.y_min..self.bounds.y_max
        ) {
            let [r, _, _, a] = self
                .img
                .get_pixel(
                    modified_pixel(x, offset_x, self.bounds.x_max),
                    modified_pixel(0, y, self.bounds.y_max),
                )
                .to_rgba()
                .data;

            let (b, g) = (
                self.img
                    .get_pixel(
                        modified_pixel(0, x, self.bounds.x_max),
                        modified_pixel(0, y, self.bounds.y_max),
                    )
                    .to_rgba()
                    .data[1],
                self.img
                    .get_pixel(
                        modified_pixel(x, offset_x, self.bounds.x_max),
                        modified_pixel(0, y, self.bounds.y_max),
                    )
                    .to_rgba()
                    .data[2],
            );
            self.buffer.put_pixel(
                x,
                y,
                Rgba([
                    brighten_pixels(r, cfg.brighteness_addition),
                    brighten_pixels(g, cfg.brighteness_addition),
                    brighten_pixels(b, cfg.brighteness_addition),
                    brighten_pixels(a, cfg.brighteness_addition),
                ]),
            );
        }
        self
    }
    fn write(&self, path: PathBuf) -> std::io::Result<()> {
        self.buffer.save(path)
    }
}

fn main() -> std::io::Result<()> {
    // Parse options from CLI
    let cli_options = CLI::from_args();
    let mut cruster = Corrupter::new(&cli_options.image);
    let mut rng = rand::thread_rng();
    cruster
        .dissolve_block(&mut rng, &cli_options)
        .random_brightening(&mut rng, &cli_options)
        .chromatic_abberations(&mut rng, &cli_options)
        .write(cli_options.output)
}
