use image::{GenericImageView, ImageBuffer, Pixel, Rgba, RgbaImage};
use rand::distributions::StandardNormal;
use rand::prelude::*;
use std::cmp::Ordering;
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
    #[structopt(default_value = "7.0")]
    magnitude: Option<f64>,
    /// Average height of a distorted block
    #[structopt(default_value = "10")]
    block_height: Option<u32>,
    /// Average offset of a distorted block
    #[structopt(default_value = "30.0")]
    block_offset: Option<f64>,
    /// Strength of the distorted block stride
    #[structopt(default_value = "0.1")]
    stride_magnitude: Option<f64>,
    /// Per-channel scanline lag strength
    #[structopt(default_value = "0.005")]
    lag: Option<f64>,
    /// Initial red scanline lag
    #[structopt(default_value = "-7.")]
    lr: Option<f64>,
    /// Initial green scanline lag
    #[structopt(default_value = "0.")]
    lg: Option<f64>,
    /// Initial blue scanline lag
    #[structopt(default_value = "3.")]
    lb: Option<f64>,
    /// Standard deviation of the red-blue channel offset (non-destructive)
    #[structopt(default_value = "10.")]
    std_offset: Option<f64>,
    /// Additional brightness control
    #[structopt(default_value = "37")]
    brighteness_addition: Option<f64>,
    /// Mean chromatic abberation offset
    #[structopt(default_value = "10")]
    mean_abberation: Option<u32>,
    /// Standard deviation of the chromatic abberation offset (lower values induce longer trails)
    #[structopt(default_value = "10.")]
    std_abberation: Option<f64>,
}

/// Dummy structure holding the four dimensiosn of the input image
struct Bounds {
    x_min: u32,
    y_min: u32,
    x_max: u32,
    y_max: u32,
}

/// Structure holding the image buffer for encapsulation purposes
struct Corrupter {
    bounds: Bounds,
    buffer: RgbaImage,
}

impl Corrupter {
    fn new(src_img: DynamicImage) -> Self {
        let bounds = src_img.bounds();
        Self {
            bounds,
            img_buf: ImageBuffer::new(bounds.x_max, bounds.y_max),
        }
    }

    fn weighted_random(weight: f64, rng: &mut ThreadRng) -> f64 {
        weight * StandardNormal.sample(rng)
    }

    /// First stage of the corrupter
    /// Goes through the image and offset some pixel spatially by blocks
    fn dissolve_block(&mut self, rng: &mut ThreadRng, cfg: StructOpt) {
        let mut line_offset = 0;
        let mut stride = 0.;
        let mut yset = 0;

        for (x, y) in iproduct!(
            self.bounds.x_min..self.bounds.x_max,
            self.bounds.y_min..self.bounds.y_max
        ) {
            if rng.gen_ratio(self.block_height, self.bounds.x_max) {
                line_offset = offset::<i64>(cfg.block_offset, rng);
                stride = weighted_random(cfg.stride_magnitude, rng);
                yset = y;
            }
            let stride_offset: i64 = (stride * (y - yset));
            let offset_x = offset::<i64>(cfg.magnitude) + cfg.line_offset + stride_offset;
            let offset_y = offset::<i64>(cfg.magnitude);
            let dissolved = self.img_buf.get_pixel(
                x + offset_x % self.bounds.x_max,
                y + offset_y % self.bounds.y_max,
            );
            self.img_buf.put_pixel(x, y, dissolved);
        }
    }
    fn random_brightening(&mut self, rng: &ThreadRng, cfg: StructOpt) {
        for (x, y) in iproduct!(
            self.bounds.x_min..self.bounds.x_max,
            self.bounds.y_min..self.bounds.y_max
        ) {
            (cfg.la, cfg.lr, cfg.lg, cfg.lb) += (
                weighted_random(cfg.la, rng),
                weighted_random(cfg.la, rng),
                weighted_random(cfg.la, rng),
            );
            offset_x = offset::<i64>(cfg.std_offset);
            let [r, _, _, a] = self
                .img_buf
                .get_pixel(
                    x + cfg.lr - offset_x % self.bounds.x_max,
                    y % self.bounds.y_max,
                )
                .to_rgba()
                .data;

            let (b, g) = (
                self.img_buf
                    .get_pixel(x + cfg.lg % self.bounds.x_max, y % self.bounds.y_max)
                    .to_rgba()
                    .data[1],
                self.img_buf
                    .get_pixel(
                        x + cfg.lb + offset_x % self.bounds.x_max,
                        y % self.bounds.y_max,
                    )
                    .to_rgba()
                    .data[2],
            );
            self.img_buf.put_pixel(x, y, Rgba([r, g, b, a]));
            self.img.buf = self.img_buf.brighten(cfg.brighteness_addition);
        }
    }
    fn chromatic_abberations(&mut self, rng: &ThreadRng, cfg: StructOpt) {
        let offset_x = mean_abberation + offset::<i64>(cfg.std_abberation);
        for (x, y) in iproduct!(
            self.bounds.x_min..self.bounds.x_max,
            self.bounds.y_min..self.bounds.y_max
        ) {
            let [r, _, _, a] = self
                .img_buf
                .get_pixel(x + cfg.offset_x % self.bounds.x_max, y % self.bounds.y_max)
                .to_rgba()
                .data;

            let (b, g) = (
                self.img_buf
                    .get_pixel(x % self.bounds.x_max, y % self.bounds.y_max)
                    .to_rgba()
                    .data[1],
                self.img_buf
                    .get_pixel(x - cfg.offset_x % self.bounds.x_max, y % self.bounds.y_max)
                    .to_rgba()
                    .data[2],
            );
            self.img_buf.put_pixel(x, y, Rgba([r, g, b, a]));
        }
    }
    fn write() {}
}

/// A map between the magnitude and the pixel spatial shift
fn offset<T>(magnitude: f64, rng: &mut ThreadRng) -> T {
    (StandardNormal.sample(rng) * magnitude) as T
}

fn main() {
    // Parse options from CLI
    let cli_options = CLI::from_args();
    let mut cruster = Corrupter::new(cli_options.image);
    let mut rng = rand::thread_rng();
    cruster
        .dissolve_block(&mut rng, cli_options)
        .random_brightening(&mut rng, cli_options)
        .chromatic_abberations(&mut rng, cli_options)
        .write(path);
}
